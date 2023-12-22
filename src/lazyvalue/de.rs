use super::owned::OwnedLazyValue;
use super::value::LazyValue;
use ::serde::de;
use ::serde::de::Visitor;
use ::serde::Deserialize;
use ::serde::Deserializer;
use faststr::FastStr;
use std::marker::PhantomData;

impl<'de: 'a, 'a> Deserialize<'de> for LazyValue<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LazyValueVisitor<'a> {
            _marker: PhantomData<LazyValue<'a>>,
        }

        impl<'de: 'a, 'a> Visitor<'de> for LazyValueVisitor<'a> {
            type Value = LazyValue<'a>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "any valid JSON value")
            }

            // TRICK: used for pass the string which has escaped chars
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LazyValue::new(FastStr::new(v).into()))
            }

            // borrowed from origin json
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LazyValue::new(v.into()))
            }
        }

        let visit = LazyValueVisitor {
            _marker: PhantomData,
        };
        deserializer.deserialize_newtype_struct(super::TOKEN, visit)
    }
}

impl<'de> Deserialize<'de> for OwnedLazyValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LazyValueVisitor;

        impl<'de> Visitor<'de> for LazyValueVisitor {
            type Value = OwnedLazyValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "any valid JSON value")
            }

            // TRICK: used for pass the string which has escaped chars
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OwnedLazyValue::new(FastStr::new(v).into()))
            }

            // borrowed from origin json
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OwnedLazyValue::new(v.into()))
            }
        }

        let visit = LazyValueVisitor;
        deserializer.deserialize_newtype_struct(super::TOKEN, visit)
    }
}
