#![feature(allocator_api)]

#[allow(unused_extern_crates)]
extern crate self as serde_alloc;

mod r#box;
mod forward;
mod hash_map;
mod native;
mod option;
mod vec;

use std::{alloc::Allocator, marker::PhantomData};

use serde_core::{Deserializer, de::DeserializeSeed};

pub use self::native::Native;
pub use serde_alloc_derive::DeserializeWithAlloc;

pub trait DeserializeWithAlloc<'de, A: Allocator + Clone>: Sized {
    fn deserialize_with_alloc<D>(deserializer: D, alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

pub struct WithAllocSeed<T, A: Allocator + Clone> {
    alloc: A,
    _marker: PhantomData<fn() -> T>,
}

impl<T, A: Allocator + Clone> WithAllocSeed<T, A> {
    pub fn new(alloc: A) -> Self {
        Self {
            alloc,
            _marker: PhantomData,
        }
    }
}

impl<'de, T, A> DeserializeSeed<'de> for WithAllocSeed<T, A>
where
    A: Allocator + Clone,
    T: DeserializeWithAlloc<'de, A>,
{
    type Value = T;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_with_alloc(deserializer, self.alloc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        alloc::{AllocError, Global, Layout},
        cell::Cell,
        ptr::NonNull,
    };

    #[derive(Clone, Default)]
    struct Tracking {
        bytes: Cell<usize>,
        objects: Cell<usize>,
    }

    unsafe impl Allocator for &Tracking {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            let p = Global.allocate(layout)?;
            self.bytes.update(|bytes| bytes + layout.size());
            self.objects.update(|objects| objects + 1);
            Ok(p)
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { Global.deallocate(ptr, layout) };
        }
    }

    fn from_json<'de, T, A>(s: &'de str, alloc: A) -> T
    where
        A: Allocator + Clone,
        T: DeserializeWithAlloc<'de, A>,
    {
        let mut de = serde_json::Deserializer::from_str(s);
        let v = T::deserialize_with_alloc(&mut de, alloc).expect("deserialize");
        de.end().expect("trailing");
        v
    }

    #[test]
    fn native_scalar() {
        let Native::<i32>(n) = from_json("42", Global);
        assert_eq!(n, 42);
    }

    #[test]
    fn vec_of_native() {
        let alloc = Tracking::default();
        let v: Vec<Native<i32>, _> = from_json("[1,2,3,4]", &alloc);
        assert_eq!(v.iter().map(|n| n.0).collect::<Vec<_>>(), [1, 2, 3, 4]);
        assert_eq!(alloc.bytes.get(), 4 * size_of::<i32>());
        assert_eq!(alloc.objects.get(), 1);
    }

    #[test]
    fn nested_vec_shares_allocator() {
        let alloc = Tracking::default();
        let v: Vec<Vec<Native<u8>, _>, _> = from_json("[[1,2],[3],[],[4,5,6]]", &alloc);
        v.into_iter()
            .eq([vec![1, 2], vec![3], vec![], vec![4, 5, 6]]);
        assert_eq!(alloc.objects.get(), 4);
    }

    #[test]
    fn option_boxed_native() {
        let alloc = Tracking::default();
        assert!(from_json::<Option<Box<Native<i32>, _>>, _>("null", &alloc).is_none());
        let some: Option<Box<Native<i32>, _>> = from_json("7", &alloc);
        assert_eq!(some, Some(Box::new_in(Native(7), &alloc)));
    }

    #[test]
    fn assert_impl() {
        #[derive(Clone)]
        struct Noop;

        unsafe impl Allocator for Noop {
            fn allocate(&self, _: Layout) -> Result<NonNull<[u8]>, AllocError> {
                todo!()
            }

            unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {
                todo!()
            }
        }

        fn assert_impl<'de, T, A>()
        where
            A: Allocator + Clone,
            T: DeserializeWithAlloc<'de, A>,
        {
        }

        assert_impl::<Option<Box<Vec<Native<Box<i32, Global>>, Noop>, Noop>>, Noop>();
    }

    #[test]
    fn derive_struct() {
        #[derive(Debug, PartialEq, serde::Deserialize)]
        enum Color {
            Red,
            Green,
            Blue,
        }

        #[derive(DeserializeWithAlloc)]
        struct Foo<A: Allocator + Clone> {
            xs: Vec<Native<i32>, A>,
            n: Native<u8>,
            boxed: Box<Native<i64>, A>,
            #[deserialize_with_alloc(native)]
            color: Color,
        }

        let alloc = Tracking::default();
        let foo: Foo<_> = from_json(r#"{"xs":[1,2,3],"n":7,"boxed":42,"color":"Green"}"#, &alloc);
        assert_eq!(foo.xs.iter().map(|n| n.0).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(foo.n.0, 7);
        assert_eq!(foo.boxed.0, 42);
        assert_eq!(foo.color, Color::Green);
        // 1 Vec backing + 1 Box = 2 allocations through the threaded allocator.
        assert_eq!(alloc.objects.get(), 2);
    }

    #[test]
    fn derive_struct_serde_skip() {
        #[derive(DeserializeWithAlloc)]
        struct Foo<A: Allocator + Clone> {
            xs: Vec<Native<i32>, A>,
            #[serde(skip)]
            cached: u32,
            #[serde(skip_deserializing)]
            other: String,
        }

        let foo: Foo<_> = from_json(r#"{"xs":[1,2,3]}"#, Global);
        assert_eq!(foo.xs.iter().map(|n| n.0).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(foo.cached, 0);
        assert_eq!(foo.other, "");
    }

    #[test]
    fn derive_struct_default_in() {
        fn fresh<A: Allocator + Clone>(alloc: A) -> Vec<u32, A> {
            let mut v = Vec::new_in(alloc);
            v.push(99);
            v
        }

        #[derive(DeserializeWithAlloc)]
        struct Foo<A: Allocator + Clone> {
            xs: Vec<Native<i32>, A>,
            #[deserialize_with_alloc(default_in = "fresh")]
            cache: Vec<u32, A>,
        }

        let alloc = Tracking::default();
        let foo: Foo<_> = from_json(r#"{"xs":[1,2]}"#, &alloc);
        assert_eq!(foo.xs.iter().map(|n| n.0).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(foo.cache.len(), 1);
        assert_eq!(foo.cache[0], 99);
    }

    #[test]
    fn derive_struct_native_container() {
        #[derive(Debug, PartialEq, serde::Deserialize, DeserializeWithAlloc)]
        #[deserialize_with_alloc(native)]
        struct Bar {
            x: i32,
            y: String,
        }

        let bar: Bar = from_json(r#"{"x":42,"y":"hello"}"#, Global);
        assert_eq!(
            bar,
            Bar {
                x: 42,
                y: "hello".into(),
            }
        );
    }
}
