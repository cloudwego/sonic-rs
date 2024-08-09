use std::fmt;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy)]
pub struct Array;

impl Serialize for Array {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [(); 0].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Array {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Array;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("empty array")
            }

            fn visit_seq<V>(self, _: V) -> Result<Array, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                Ok(Array)
            }
        }

        deserializer.deserialize_tuple(0, Visitor)
    }
}
