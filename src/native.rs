use std::alloc::Allocator;

use serde_core::{Deserialize, Deserializer};

use crate::DeserializeWithAlloc;

/// A transparent wrapper that adapts any type implementing [`Deserialize`]
/// into a [`DeserializeWithAlloc`] (the allocator is ignored).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Native<T>(pub T);

impl<T> Native<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: PartialEq> PartialEq<T> for Native<T> {
    fn eq(&self, other: &T) -> bool {
        self.0.eq(other)
    }
}

impl<'de, T, A> DeserializeWithAlloc<'de, A> for Native<T>
where
    A: Allocator + Clone,
    T: Deserialize<'de>,
{
    fn deserialize_with_alloc<D>(deserializer: D, _alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Native)
    }
}
