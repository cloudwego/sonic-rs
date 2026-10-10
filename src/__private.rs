//! Implementation details used by `SonicDeserialize` expansions.
//!
//! This module is public only because proc-macro output is compiled in the
//! downstream crate. It is not a stable user-facing interface.

use core::marker::PhantomData;

pub use phf;
use serde::de::{Deserialize, Deserializer, Error, Visitor};

/// Preserve Serde's missing-field behavior: a missing `Option<T>` is `None`,
/// while every other missing required type produces `Error::missing_field`.
pub fn missing_field<'de, V, E>(field: &'static str) -> Result<V, E>
where
    V: Deserialize<'de>,
    E: Error,
{
    struct MissingFieldDeserializer<E>(&'static str, PhantomData<E>);

    impl<'de, E> Deserializer<'de> for MissingFieldDeserializer<E>
    where
        E: Error,
    {
        type Error = E;

        fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, E>
        where
            V: Visitor<'de>,
        {
            Err(Error::missing_field(self.0))
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, E>
        where
            V: Visitor<'de>,
        {
            visitor.visit_none()
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf unit unit_struct newtype_struct seq tuple tuple_struct
            map struct enum identifier ignored_any
        }
    }

    V::deserialize(MissingFieldDeserializer(field, PhantomData))
}
