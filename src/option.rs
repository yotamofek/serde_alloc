use std::{
    alloc::Allocator,
    fmt::{self, Formatter},
    marker::PhantomData,
};

use serde_core::{
    Deserializer,
    de::{
        self, EnumAccess, Error, IntoDeserializer, MapAccess, SeqAccess,
        value::{EnumAccessDeserializer, MapAccessDeserializer, SeqAccessDeserializer},
    },
};

use crate::DeserializeWithAlloc;

struct Visitor<T, A: Allocator + Clone> {
    alloc: A,
    _marker: PhantomData<fn() -> T>,
}

macro_rules! forward_primitive {
    ($($visit:ident($ty:ty),)*) => {
        $(
            #[inline]
            fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
            where
                E: Error,
            {
                T::deserialize_with_alloc(IntoDeserializer::<'de, E>::into_deserializer(v), self.alloc)
                    .map(Some)
            }
        )*
    };
}

impl<'de, T, A> de::Visitor<'de> for Visitor<T, A>
where
    T: DeserializeWithAlloc<'de, A>,
    A: Allocator + Clone,
{
    type Value = Option<T>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "option")
    }

    #[inline]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(None)
    }

    #[inline]
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(None)
    }

    #[inline]
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_with_alloc(deserializer, self.alloc).map(Some)
    }

    #[inline]
    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_with_alloc(deserializer, self.alloc).map(Some)
    }

    forward_primitive! {
        visit_bool(bool),
        visit_i8(i8),
        visit_i16(i16),
        visit_i32(i32),
        visit_i64(i64),
        visit_i128(i128),
        visit_u8(u8),
        visit_u16(u16),
        visit_u32(u32),
        visit_u64(u64),
        visit_u128(u128),
        visit_f32(f32),
        visit_f64(f64),
        visit_char(char),
        visit_str(&str),
        visit_borrowed_str(&'de str),
        visit_string(String),
        visit_bytes(&[u8]),
        visit_borrowed_bytes(&'de [u8]),
        visit_byte_buf(Vec<u8>),
    }

    #[inline]
    fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        T::deserialize_with_alloc(SeqAccessDeserializer::new(seq), self.alloc).map(Some)
    }

    #[inline]
    fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        T::deserialize_with_alloc(MapAccessDeserializer::new(map), self.alloc).map(Some)
    }

    #[inline]
    fn visit_enum<En>(self, data: En) -> Result<Self::Value, En::Error>
    where
        En: EnumAccess<'de>,
    {
        T::deserialize_with_alloc(EnumAccessDeserializer::new(data), self.alloc).map(Some)
    }
}

impl<'de, T, A> DeserializeWithAlloc<'de, A> for Option<T>
where
    A: Allocator + Clone,
    T: DeserializeWithAlloc<'de, A>,
{
    fn deserialize_with_alloc<D>(deserializer: D, alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_option(Visitor {
            alloc,
            _marker: PhantomData,
        })
    }
}
