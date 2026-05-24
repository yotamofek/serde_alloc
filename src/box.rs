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
                T::deserialize_with_alloc(IntoDeserializer::<'de, E>::into_deserializer(v), self.alloc.clone())
                    .map(|val| Box::new_in(val, self.alloc))
            }
        )*
    };
}

impl<'de, T, A> de::Visitor<'de> for Visitor<T, A>
where
    T: DeserializeWithAlloc<'de, A>,
    A: Allocator + Clone,
{
    type Value = Box<T, A>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "box")
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

    fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        T::deserialize_with_alloc(SeqAccessDeserializer::new(seq), self.alloc.clone())
            .map(|val| Box::new_in(val, self.alloc))
    }

    fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        T::deserialize_with_alloc(MapAccessDeserializer::new(map), self.alloc.clone())
            .map(|val| Box::new_in(val, self.alloc))
    }

    fn visit_enum<En>(self, data: En) -> Result<Self::Value, En::Error>
    where
        En: EnumAccess<'de>,
    {
        T::deserialize_with_alloc(EnumAccessDeserializer::new(data), self.alloc.clone())
            .map(|val| Box::new_in(val, self.alloc))
    }
}

impl<'de, T, A> DeserializeWithAlloc<'de, A> for Box<T, A>
where
    T: DeserializeWithAlloc<'de, A>,
    A: Allocator + Clone,
{
    fn deserialize_with_alloc<D>(deserializer: D, alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(Visitor {
            alloc,
            _marker: PhantomData,
        })
    }
}
