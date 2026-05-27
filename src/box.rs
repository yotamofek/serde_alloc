use std::alloc::Allocator;

use serde_core::Deserializer;

use crate::DeserializeWithAlloc;

impl<'de, T, A> DeserializeWithAlloc<'de, A> for Box<T, A>
where
    T: DeserializeWithAlloc<'de, A>,
    A: Allocator + Clone,
{
    #[inline]
    fn deserialize_with_alloc<D>(deserializer: D, alloc: A) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_with_alloc(deserializer, alloc.clone()).map(
            #[inline]
            |val| Box::new_in(val, alloc),
        )
    }
}
