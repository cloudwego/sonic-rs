use std::marker::PhantomData;

use ::serde::{de, de::Visitor, Deserialize, Deserializer};
use faststr::FastStr;

use super::{owned::OwnedLazyValue, value::LazyValue};
use crate::{lazyvalue::value::HasEsc, serde::hand_off};

impl<'de: 'a, 'a> Deserialize<'de> for LazyValue<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LazyValueVisitor<'a> {
            _marker: PhantomData<&'a ()>,
        }

        impl<'de: 'a, 'a> Visitor<'de> for LazyValueVisitor<'a> {
            type Value = LazyValue<'a>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "any valid JSON value")
            }

            // NOTE: only used for visit the str that has escaped chars
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LazyValue::new(FastStr::new(v).into(), HasEsc::Yes))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LazyValue::new(v.as_bytes().into(), HasEsc::None))
            }
        }

        let visit = LazyValueVisitor {
            _marker: PhantomData,
        };
        deserializer.deserialize_newtype_struct(super::TOKEN, visit)
    }
}

pub(crate) struct OwnedVisitor;

impl<'de> Visitor<'de> for OwnedVisitor {
    type Value = OwnedLazyValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("expect a valid json")
    }

    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        hand_off::take(v).ok_or_else(|| E::invalid_type(de::Unexpected::Bytes(v), &self))
    }
}

impl<'de> Deserialize<'de> for OwnedLazyValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_newtype_struct(super::OWNED_LAZY_VALUE_TOKEN, OwnedVisitor)
    }
}
