use std::alloc::Allocator;

use serde_core::{Deserialize, Deserializer};

use crate::DeserializeWithAlloc;

macro_rules! forward {
    ($($ty:ty),+) => {
        $(
            impl<'de, A: Allocator + Clone> DeserializeWithAlloc<'de, A> for $ty {
                #[inline(always)]
                fn deserialize_with_alloc<D>(deserializer: D, _: A) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    <$ty as Deserialize>::deserialize(deserializer)
                }
            }
        )+
    };
}

forward!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, String, bool, char
);
