use super::LazyValue;
use faststr::FastStr;
use serde::de;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;

impl<'de> Deserialize<'de> for LazyValue<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LazyValueVisitor;

        impl<'de> Visitor<'de> for LazyValueVisitor {
            type Value = LazyValue<'de>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "any valid JSON value")
            }

            // copy into a faststr
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

        deserializer.deserialize_newtype_struct(super::TOKEN, LazyValueVisitor)
    }
}
