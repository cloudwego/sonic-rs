use std::{fmt, fmt::Display, str::FromStr};

use serde::{
    de::{self, Deserialize, Deserializer, Unexpected},
    ser::{Serialize, Serializer},
};

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct PrimStr<T>(T)
where
    T: Copy + Ord + Display + FromStr;

impl<T> Serialize for PrimStr<T>
where
    T: Copy + Ord + Display + FromStr,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

impl<'de, T> Deserialize<'de> for PrimStr<T>
where
    T: Copy + Ord + Display + FromStr,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::marker::PhantomData;
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for Visitor<T>
        where
            T: Copy + Ord + Display + FromStr,
        {
            type Value = PrimStr<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("number represented as string")
            }

            fn visit_str<E>(self, value: &str) -> Result<PrimStr<T>, E>
            where
                E: de::Error,
            {
                match T::from_str(value) {
                    Ok(id) => Ok(PrimStr(id)),
                    Err(_) => Err(E::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_str(Visitor(PhantomData))
    }
}
