// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.

use crate::util::num::ParserNumber;
use crate::util::private::Sealed;
use serde::de::value::BorrowedStrDeserializer;
use serde::de::MapAccess;
use serde::ser::SerializeStruct;
/// Represents a JSON number, whether integer or floating point.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Number {
    n: N,
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum N {
    PosInt(u64),
    /// Always less than zero.
    NegInt(i64),
    /// Always finite.
    Float(f64),
}

use crate::error::Error;
use core::fmt::{self, Debug, Display};
use core::hash::{Hash, Hasher};
use serde::de::{self, Unexpected, Visitor};
use serde::{forward_to_deserialize_any, Deserialize, Deserializer, Serialize, Serializer};

impl PartialEq for N {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (N::PosInt(a), N::PosInt(b)) => a == b,
            (N::NegInt(a), N::NegInt(b)) => a == b,
            (N::Float(a), N::Float(b)) => a == b,
            _ => false,
        }
    }
}

// Implementing Eq is fine since any float values are always finite.
impl Eq for N {}

impl Hash for N {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match *self {
            N::PosInt(i) => i.hash(h),
            N::NegInt(i) => i.hash(h),
            N::Float(f) => {
                if f == 0.0f64 {
                    // There are 2 zero representations, +0 and -0, which
                    // compare equal but have different bits. We use the +0 hash
                    // for both so that hash(+0) == hash(-0).
                    0.0f64.to_bits().hash(h);
                } else {
                    f.to_bits().hash(h);
                }
            }
        }
    }
}

pub trait JsonNumberTrait: Sealed {
    fn is_i64(&self) -> bool;
    fn is_u64(&self) -> bool;
    fn is_f64(&self) -> bool;
    fn as_i64(&self) -> Option<i64>;
    fn as_u64(&self) -> Option<u64>;
    fn as_f64(&self) -> Option<f64>;
}

impl Sealed for Number {}

impl JsonNumberTrait for Number {
    /// Returns true if the `Number` is an integer between `i64::MIN` and
    /// `i64::MAX`.
    ///
    /// For any Number on which `is_i64` returns true, `as_i64` is guaranteed to
    /// return the integer value.
    #[inline]
    fn is_i64(&self) -> bool {
        match self.n {
            N::PosInt(v) => v <= i64::max_value() as u64,
            N::NegInt(_) => true,
            N::Float(_) => false,
        }
    }

    /// Returns true if the `Number` is an integer between zero and `u64::MAX`.
    ///
    /// For any Number on which `is_u64` returns true, `as_u64` is guaranteed to
    /// return the integer value.
    #[inline]
    fn is_u64(&self) -> bool {
        match self.n {
            N::PosInt(_) => true,
            N::NegInt(_) | N::Float(_) => false,
        }
    }

    /// Returns true if the `Number` can be represented by f64.
    ///
    /// For any Number on which `is_f64` returns true, `as_f64` is guaranteed to
    /// return the floating point value.
    ///
    /// Currently this function returns true if and only if both `is_i64` and
    /// `is_u64` return false but this is not a guarantee in the future.
    ///
    #[inline]
    fn is_f64(&self) -> bool {
        match self.n {
            N::Float(_) => true,
            N::PosInt(_) | N::NegInt(_) => false,
        }
    }

    /// If the `Number` is an integer, represent it as i64 if possible. Returns
    /// None otherwise.
    #[inline]
    fn as_i64(&self) -> Option<i64> {
        match self.n {
            N::PosInt(n) => {
                if n <= i64::max_value() as u64 {
                    Some(n as i64)
                } else {
                    None
                }
            }
            N::NegInt(n) => Some(n),
            N::Float(_) => None,
        }
    }

    /// If the `Number` is an integer, represent it as u64 if possible. Returns
    /// None otherwise.
    #[inline]
    fn as_u64(&self) -> Option<u64> {
        match self.n {
            N::PosInt(n) => Some(n),
            N::NegInt(_) | N::Float(_) => None,
        }
    }

    /// Represents the number as finite f64 if possible. Returns None otherwise.
    #[inline]
    fn as_f64(&self) -> Option<f64> {
        match self.n {
            N::PosInt(n) => Some(n as f64),
            N::NegInt(n) => Some(n as f64),
            N::Float(n) => Some(n),
        }
    }
}

impl Number {
    /// Converts a finite `f64` to a `Number`. Infinite or NaN values are not JSON
    /// numbers.
    ///
    #[inline]
    pub fn from_f64(f: f64) -> Option<Number> {
        if f.is_finite() {
            let n = { N::Float(f) };
            Some(Number { n })
        } else {
            None
        }
    }
}

impl Display for Number {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.n {
            N::PosInt(u) => formatter.write_str(itoa::Buffer::new().format(u)),
            N::NegInt(i) => formatter.write_str(itoa::Buffer::new().format(i)),
            N::Float(f) => formatter.write_str(ryu::Buffer::new().format_finite(f)),
        }
    }
}

impl Debug for Number {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Number({})", self)
    }
}

impl Serialize for Number {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.n {
            N::PosInt(u) => serializer.serialize_u64(u),
            N::NegInt(i) => serializer.serialize_i64(i),
            N::Float(f) => serializer.serialize_f64(f),
        }
    }
}

impl<'de> Deserialize<'de> for Number {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Number, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NumberVisitor;

        impl<'de> Visitor<'de> for NumberVisitor {
            type Value = Number;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON number")
            }

            #[inline]
            fn visit_i64<E>(self, value: i64) -> Result<Number, E> {
                Ok(value.into())
            }

            #[inline]
            fn visit_u64<E>(self, value: u64) -> Result<Number, E> {
                Ok(value.into())
            }

            #[inline]
            fn visit_f64<E>(self, value: f64) -> Result<Number, E>
            where
                E: de::Error,
            {
                Number::from_f64(value).ok_or_else(|| de::Error::custom("not a JSON number"))
            }
        }

        deserializer.deserialize_any(NumberVisitor)
    }
}

macro_rules! deserialize_any {
    (@expand [$($num_string:tt)*]) => {
        #[inline]
        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.n {
                N::PosInt(u) => visitor.visit_u64(u),
                N::NegInt(i) => visitor.visit_i64(i),
                N::Float(f) => visitor.visit_f64(f),
            }
        }
    };

    (owned) => {
        deserialize_any!(@expand [n]);
    };

    (ref) => {
        deserialize_any!(@expand [n.clone()]);
    };
}

macro_rules! deserialize_number {
    ($deserialize:ident => $visit:ident) => {
        fn $deserialize<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_any(visitor)
        }
    };
}

impl<'de> Deserializer<'de> for Number {
    type Error = Error;

    deserialize_any!(owned);

    deserialize_number!(deserialize_i8 => visit_i8);
    deserialize_number!(deserialize_i16 => visit_i16);
    deserialize_number!(deserialize_i32 => visit_i32);
    deserialize_number!(deserialize_i64 => visit_i64);
    deserialize_number!(deserialize_i128 => visit_i128);
    deserialize_number!(deserialize_u8 => visit_u8);
    deserialize_number!(deserialize_u16 => visit_u16);
    deserialize_number!(deserialize_u32 => visit_u32);
    deserialize_number!(deserialize_u64 => visit_u64);
    deserialize_number!(deserialize_u128 => visit_u128);
    deserialize_number!(deserialize_f32 => visit_f32);
    deserialize_number!(deserialize_f64 => visit_f64);

    forward_to_deserialize_any! {
        bool char str string bytes byte_buf option unit unit_struct
        newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

impl<'de, 'a> Deserializer<'de> for &'a Number {
    type Error = Error;

    deserialize_any!(ref);

    deserialize_number!(deserialize_i8 => visit_i8);
    deserialize_number!(deserialize_i16 => visit_i16);
    deserialize_number!(deserialize_i32 => visit_i32);
    deserialize_number!(deserialize_i64 => visit_i64);
    deserialize_number!(deserialize_i128 => visit_i128);
    deserialize_number!(deserialize_u8 => visit_u8);
    deserialize_number!(deserialize_u16 => visit_u16);
    deserialize_number!(deserialize_u32 => visit_u32);
    deserialize_number!(deserialize_u64 => visit_u64);
    deserialize_number!(deserialize_u128 => visit_u128);
    deserialize_number!(deserialize_f32 => visit_f32);
    deserialize_number!(deserialize_f64 => visit_f64);

    forward_to_deserialize_any! {
        bool char str string bytes byte_buf option unit unit_struct
        newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

impl From<ParserNumber> for Number {
    fn from(value: ParserNumber) -> Self {
        let n = match value {
            ParserNumber::Float(f) => N::Float(f),
            ParserNumber::Unsigned(u) => N::PosInt(u),
            ParserNumber::Signed(i) => N::NegInt(i),
        };
        Number { n }
    }
}

macro_rules! impl_from_unsigned {
    (
        $($ty:ty),*
    ) => {
        $(
            impl From<$ty> for Number {
                #[inline]
                fn from(u: $ty) -> Self {
                    let n = {
                        { N::PosInt(u as u64) }
                    };
                    Number { n }
                }
            }
        )*
    };
}

macro_rules! impl_from_signed {
    (
        $($ty:ty),*
    ) => {
        $(
            impl From<$ty> for Number {
                #[inline]
                fn from(i: $ty) -> Self {
                    let n = {
                            if i < 0 {
                                N::NegInt(i as i64)
                            } else {
                                N::PosInt(i as u64)
                            }
                    };
                    Number { n }
                }
            }
        )*
    };
}

impl_from_unsigned!(u8, u16, u32, u64, usize);
impl_from_signed!(i8, i16, i32, i64, isize);

/// Represents a JSON number with arbitrary precision, like as Golang json.Number
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RawNumber {
    n: String,
}

impl RawNumber {
    pub(crate) fn new(s: &str) -> Self {
        Self { n: s.to_string() }
    }

    /// as_str returns the underlying string representation of the number.
    pub fn as_str(&self) -> &str {
        self.n.as_str()
    }
}

pub(crate) const TOKEN: &str = "$sonic_rs::private::JsonNumber";

impl<'de> Deserialize<'de> for RawNumber {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<RawNumber, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct JsonNumberVisitor;

        impl<'de> Visitor<'de> for JsonNumberVisitor {
            type Value = RawNumber;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON number")
            }

            #[inline]
            fn visit_map<V>(self, mut visitor: V) -> Result<RawNumber, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                // use raw value
                let value = visitor.next_key::<JsonNumberKey>()?;
                if value.is_none() {
                    return Err(de::Error::invalid_type(Unexpected::Map, &self));
                }
                visitor.next_value_seed(JsonNumberString)
            }
        }

        deserializer.deserialize_newtype_struct(TOKEN, JsonNumberVisitor)
    }
}

pub(crate) struct JsonNumberString;

impl<'de> de::DeserializeSeed<'de> for JsonNumberString {
    type Value = RawNumber;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for JsonNumberString {
    type Value = RawNumber;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("json number")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RawNumber { n: s.to_string() })
    }

    fn visit_string<E>(self, s: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RawNumber { n: s })
    }
}

struct JsonNumberKey;

impl<'de> de::Deserialize<'de> for JsonNumberKey {
    fn deserialize<D>(deserializer: D) -> Result<JsonNumberKey, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct FieldVisitor;

        impl<'de> de::Visitor<'de> for FieldVisitor {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid number field")
            }

            fn visit_str<E>(self, s: &str) -> Result<(), E>
            where
                E: de::Error,
            {
                if s == TOKEN {
                    Ok(())
                } else {
                    Err(de::Error::custom("expected field with custom name"))
                }
            }
        }

        deserializer.deserialize_identifier(FieldVisitor)?;
        Ok(JsonNumberKey)
    }
}

pub struct BorrowedJsonNumberDeserializer<'de> {
    pub raw_value: Option<&'de str>,
}

impl<'de> MapAccess<'de> for BorrowedJsonNumberDeserializer<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.raw_value.is_none() {
            return Ok(None);
        }
        seed.deserialize(JsonNumberKeyDeserializer).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(BorrowedStrDeserializer::new(self.raw_value.take().unwrap()))
    }
}

struct JsonNumberKeyDeserializer;

impl<'de> Deserializer<'de> for JsonNumberKeyDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_borrowed_str(TOKEN)
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 char str string seq
        bytes byte_buf map struct option unit newtype_struct ignored_any
        unit_struct tuple_struct tuple enum identifier
    }
}

impl Serialize for RawNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ident = crate::serde::raw::TOKEN;
        let mut s = serializer.serialize_struct(ident, 1)?;
        s.serialize_field(ident, &self.n)?;
        s.end()
    }
}

impl Sealed for RawNumber {}

impl JsonNumberTrait for RawNumber {
    /// Returns true if the `Number` is an integer between `i64::MIN` and
    /// `i64::MAX`.
    ///
    /// For any Number on which `is_i64` returns true, `as_i64` is guaranteed to
    /// return the integer value.
    #[inline]
    fn is_i64(&self) -> bool {
        self.as_i64().is_some()
    }

    /// Returns true if the `Number` is an integer between zero and `u64::MAX`.
    ///
    /// For any Number on which `is_u64` returns true, `as_u64` is guaranteed to
    /// return the integer value.
    #[inline]
    fn is_u64(&self) -> bool {
        self.as_u64().is_some()
    }

    /// Returns true if the `Number` can be represented by f64.
    ///
    /// For any Number on which `is_f64` returns true, `as_f64` is guaranteed to
    /// return the floating point value.
    ///
    /// Currently this function returns true if and only if both `is_i64` and
    /// `is_u64` return false but this is not a guarantee in the future.
    ///
    #[inline]
    fn is_f64(&self) -> bool {
        self.as_f64().is_some()
    }

    /// If the `Number` is an integer, represent it as i64 if possible. Returns
    /// None otherwise.
    #[inline]
    fn as_i64(&self) -> Option<i64> {
        self.n.parse().ok()
    }

    /// If the `Number` is an integer, represent it as u64 if possible. Returns
    /// None otherwise.
    #[inline]
    fn as_u64(&self) -> Option<u64> {
        self.n.parse().ok()
    }

    /// Represents the number as finite f64 if possible. Returns None otherwise.
    #[inline]
    fn as_f64(&self) -> Option<f64> {
        self.n.parse::<f64>().ok().filter(|float| float.is_finite())
    }
}

impl TryFrom<RawNumber> for Number {
    type Error = Error;

    fn try_from(value: RawNumber) -> Result<Self, Self::Error> {
        let num: Number = crate::from_str(value.n.as_str())?;
        Ok(num)
    }
}
