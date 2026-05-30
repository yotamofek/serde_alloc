use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Field, Fields, GenericParam, TypeParam, WherePredicate, parse_macro_input,
    parse_quote,
};

fn has_native_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("deserialize_with_alloc") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("native") {
                found = true;
            } else if meta.input.peek(syn::Token![=]) {
                let _: syn::Expr = meta.value()?.parse()?;
            }
            Ok(())
        });
        found
    })
}

fn is_native(field: &Field) -> bool {
    has_native_attr(&field.attrs)
}

fn field_default_in(field: &Field) -> Option<syn::Path> {
    let mut result = None;
    for attr in &field.attrs {
        if !attr.path().is_ident("deserialize_with_alloc") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default_in") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                result = Some(lit.parse()?);
            } else if meta.input.peek(syn::Token![=]) {
                let _: syn::Expr = meta.value()?.parse()?;
            }
            Ok(())
        });
    }
    result
}

fn is_skipped(field: &Field) -> bool {
    is_serde_skipped(field) || field_default_in(field).is_some()
}

fn is_serde_skipped(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("serde"))
        .any(|attr| {
            let syn::Meta::List(list) = &attr.meta else {
                return false;
            };
            list.tokens.clone().into_iter().any(|tt| {
                matches!(
                    tt,
                    proc_macro2::TokenTree::Ident(ref ident)
                        if ident == "skip" || ident == "skip_deserializing"
                )
            })
        })
}

#[proc_macro_derive(DeserializeWithAlloc, attributes(deserialize_with_alloc, serde))]
pub fn derive_deserialize_with_alloc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let name_str = name.to_string();

    if has_native_attr(&input.attrs) {
        let mut augmented = input.generics.clone();
        augmented.params.insert(0, parse_quote!('de));
        augmented
            .params
            .push(parse_quote!(__A: ::std::alloc::Allocator + ::core::clone::Clone));
        augmented
            .make_where_clause()
            .predicates
            .push(parse_quote!(Self: ::serde::Deserialize<'de>));
        let (impl_generics, _, where_clause) = augmented.split_for_impl();
        let (_, struct_ty_generics, _) = input.generics.split_for_impl();

        return quote! {
            #[automatically_derived]
            impl #impl_generics ::serde_alloc::DeserializeWithAlloc<'de, __A>
                for #name #struct_ty_generics
            #where_clause
            {
                #[inline(always)]
                fn deserialize_with_alloc<__D>(
                    deserializer: __D,
                    _alloc: __A,
                ) -> ::core::result::Result<Self, __D::Error>
                where
                    __D: ::serde::Deserializer<'de>,
                {
                    <Self as ::serde::Deserialize<'de>>::deserialize(deserializer)
                }
            }
        }
        .into();
    }

    let alloc_ident = input
        .generics
        .params
        .iter()
        .find_map(|p| {
            let GenericParam::Type(TypeParam { ident, bounds, .. }) = p else {
                return None;
            };
            bounds
                .iter()
                .any(|b| quote!(#b).to_string().contains("Allocator"))
                .then(|| ident.clone())
        })
        .expect(
            "DeserializeWithAlloc derive requires a generic type parameter bounded by Allocator",
        );

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => &n.named,
            _ => panic!("DeserializeWithAlloc derive only supports structs with named fields"),
        },
        _ => panic!("DeserializeWithAlloc derive only supports structs"),
    };

    let active_fields: Vec<&Field> = fields.iter().filter(|f| !is_skipped(f)).collect();

    let field_idents: Vec<_> = active_fields
        .iter()
        .map(|f| f.ident.clone().unwrap())
        .collect();
    let field_strs: Vec<_> = field_idents.iter().map(|i| i.to_string()).collect();
    let field_types: Vec<_> = active_fields.iter().map(|f| &f.ty).collect();
    let field_native: Vec<bool> = active_fields.iter().copied().map(is_native).collect();

    let field_seed_types: Vec<proc_macro2::TokenStream> = active_fields
        .iter()
        .zip(&field_native)
        .map(|(f, &native)| {
            let ty = &f.ty;
            if native {
                quote!(::serde_alloc::Native<#ty>)
            } else {
                quote!(#ty)
            }
        })
        .collect();
    let field_unwrap: Vec<proc_macro2::TokenStream> = field_native
        .iter()
        .map(|&native| {
            if native {
                quote!(.into_inner())
            } else {
                quote!()
            }
        })
        .collect();

    let init_pairs: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().unwrap();
            if let Some(path) = field_default_in(f) {
                quote! {
                    #ident: #path(::core::clone::Clone::clone(&self.alloc))
                }
            } else if is_serde_skipped(f) {
                quote!(#ident: ::core::default::Default::default())
            } else {
                let name = ident.to_string();
                quote! {
                    #ident: #ident.ok_or_else(||
                        <__M::Error as ::serde::de::Error>::missing_field(#name)
                    )?
                }
            }
        })
        .collect();

    let (_, struct_ty_generics, _) = input.generics.split_for_impl();
    let struct_ty_generics_tokens = quote!(#struct_ty_generics);

    let visitor_decl_params = &input.generics.params;
    let visitor_decl_where = input.generics.where_clause.as_ref();

    let mut augmented = input.generics.clone();
    augmented.params.insert(0, parse_quote!('de));
    {
        let w = augmented.make_where_clause();
        for seed_ty in &field_seed_types {
            let pred: WherePredicate =
                parse_quote!(#seed_ty: ::serde_alloc::DeserializeWithAlloc<'de, #alloc_ident>);
            w.predicates.push(pred);
        }
    }
    let (impl_generics, _, where_clause) = augmented.split_for_impl();

    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics ::serde_alloc::DeserializeWithAlloc<'de, #alloc_ident>
            for #name #struct_ty_generics_tokens
        #where_clause
        {
            fn deserialize_with_alloc<__D>(
                deserializer: __D,
                alloc: #alloc_ident,
            ) -> ::core::result::Result<Self, __D::Error>
            where
                __D: ::serde::Deserializer<'de>,
            {
                struct __Visitor<#visitor_decl_params> #visitor_decl_where {
                    alloc: #alloc_ident,
                    _marker: ::core::marker::PhantomData<fn() -> #name #struct_ty_generics_tokens>,
                }

                impl #impl_generics ::serde::de::Visitor<'de>
                    for __Visitor #struct_ty_generics_tokens
                #where_clause
                {
                    type Value = #name #struct_ty_generics_tokens;

                    fn expecting(
                        &self,
                        formatter: &mut ::core::fmt::Formatter<'_>,
                    ) -> ::core::fmt::Result {
                        formatter.write_str(#name_str)
                    }

                    fn visit_map<__M>(
                        self,
                        mut map: __M,
                    ) -> ::core::result::Result<Self::Value, __M::Error>
                    where
                        __M: ::serde::de::MapAccess<'de>,
                    {
                        #(
                            let mut #field_idents: ::core::option::Option<#field_types> =
                                ::core::option::Option::None;
                        )*

                        while let ::core::option::Option::Some(__key) =
                            ::serde::de::MapAccess::next_key::<::std::string::String>(&mut map)?
                        {
                            match __key.as_str() {
                                #(
                                    #field_strs => {
                                        if #field_idents.is_some() {
                                            return ::core::result::Result::Err(
                                                <__M::Error as ::serde::de::Error>::duplicate_field(
                                                    #field_strs,
                                                ),
                                            );
                                        }
                                        #field_idents = ::core::option::Option::Some(
                                            ::serde::de::MapAccess::next_value_seed(
                                                &mut map,
                                                ::serde_alloc::WithAllocSeed::<#field_seed_types, _>::new(
                                                    ::core::clone::Clone::clone(&self.alloc),
                                                ),
                                            )?
                                            #field_unwrap,
                                        );
                                    }
                                )*
                                __other => {
                                    return ::core::result::Result::Err(
                                        <__M::Error as ::serde::de::Error>::unknown_field(
                                            __other,
                                            &[#(#field_strs),*],
                                        ),
                                    );
                                }
                            }
                        }

                        ::core::result::Result::Ok(Self::Value {
                            #(#init_pairs,)*
                        })
                    }
                }

                ::serde::Deserializer::deserialize_map(
                    deserializer,
                    __Visitor {
                        alloc,
                        _marker: ::core::marker::PhantomData,
                    },
                )
            }
        }
    };

    expanded.into()
}
