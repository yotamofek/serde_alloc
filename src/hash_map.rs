use std::{
    alloc::Allocator,
    collections::HashMap,
    fmt::{self, Formatter},
    hash::{BuildHasher, Hash},
    marker::PhantomData,
};

use serde_core::{
    Deserializer,
    de::{self, MapAccess},
};

use crate::{DeserializeWithAlloc, WithAllocSeed};

struct Visitor<K, V, S, A: Allocator + Clone> {
    alloc: A,
    _kv_marker: PhantomData<fn() -> (K, V)>,
    _s_marker: PhantomData<fn() -> S>,
}

impl<'de, K, V, S, A> de::Visitor<'de> for Visitor<K, V, S, A>
where
    K: DeserializeWithAlloc<'de, A> + Eq + Hash,
    V: DeserializeWithAlloc<'de, A>,
    S: BuildHasher + Default,
    A: Allocator + Clone,
{
    type Value = HashMap<K, V, S, A>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut res = HashMap::with_capacity_and_hasher_in(
            map.size_hint().unwrap_or(0),
            S::default(),
            self.alloc.clone(),
        );

        while let Some((key, value)) = map.next_entry_seed(
            WithAllocSeed::new(self.alloc.clone()),
            WithAllocSeed::new(self.alloc.clone()),
        )? {
            res.insert(key, value);
        }

        Ok(res)
    }
}

impl<'de, K, V, S, A> DeserializeWithAlloc<'de, A> for HashMap<K, V, S, A>
where
    K: DeserializeWithAlloc<'de, A> + Eq + Hash,
    V: DeserializeWithAlloc<'de, A>,
    S: BuildHasher + Default,
    A: Allocator + Clone,
{
    #[inline]
    fn deserialize_with_alloc<D>(deserializer: D, alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(Visitor {
            alloc,
            _kv_marker: PhantomData,
            _s_marker: PhantomData,
        })
    }
}
