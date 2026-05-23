use std::{
    alloc::Allocator,
    fmt::{self, Formatter},
    marker::PhantomData,
};

use serde_core::{
    Deserializer,
    de::{self, SeqAccess},
};

use crate::{DeserializeWithAlloc, WithAllocSeed};

struct Visitor<A: Allocator + Clone, T> {
    alloc: A,
    _marker: PhantomData<fn() -> T>,
}

impl<'de, A, T> de::Visitor<'de> for Visitor<A, T>
where
    A: Allocator + Clone,
    T: DeserializeWithAlloc<'de, A>,
{
    type Value = Vec<T, A>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut vec = Vec::with_capacity_in(seq.size_hint().unwrap_or(0), self.alloc.clone());

        while let Some(value) =
            seq.next_element_seed(WithAllocSeed::<T, A>::new(self.alloc.clone()))?
        {
            vec.push(value);
        }
        Ok(vec)
    }
}

impl<'de, T, A> DeserializeWithAlloc<'de, A> for Vec<T, A>
where
    A: Allocator + Clone,
    T: DeserializeWithAlloc<'de, A>,
{
    fn deserialize_with_alloc<D>(deserializer: D, alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(Visitor::<A, T> {
            alloc,
            _marker: PhantomData,
        })
    }
}
