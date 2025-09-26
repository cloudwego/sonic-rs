use std::fmt::Display;

use serde::{
    de::Unexpected,
    ser::{Impossible, Serialize},
};

use crate::{
    error::{Error, ErrorCode, Result},
    serde::ser::key_must_be_str_or_num,
    value::node::Value,
};

/// Convert a `T` into `sonic_rs::Value` which can represent any valid JSON data.
///
/// # Example
///
/// ```
/// use serde::Serialize;
/// use sonic_rs::{json, to_value, Value};
///
/// #[derive(Serialize, Debug)]
/// struct User {
///     string: String,
///     number: i32,
///     array: Vec<String>,
/// }
///
///  let user = User{
///      string: "hello".into(),
///      number: 123,
///      array: vec!["a".into(), "b".into(), "c".into()],
///  };
///  let got: Value = sonic_rs::to_value(&user).unwrap();
///  let expect = json!({
///      "string": "hello",
///      "number": 123,
///      "array": ["a", "b", "c"],
///  });
///  assert_eq!(got, expect);
/// ```
///
/// # Errors
///
/// This conversion can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
///
/// ```
/// use std::collections::BTreeMap;
///
/// use sonic_rs::to_value;
///
/// // The keys in this map are vectors, not strings.
/// let mut map = BTreeMap::new();
/// map.insert(vec![32, 64], "x86");
/// let err = to_value(&map).unwrap_err().to_string();
/// assert!(err.contains("Expected the key to be string/bool/number when serializing map"));
/// ```
pub fn to_value<T>(value: &T) -> Result<Value>
where
    T: ?Sized + Serialize,
{
    value.serialize(Serializer)
}

// Not export this because it is mainly used in `json!`.
pub(crate) struct Serializer;

use super::JsonValueTrait;
use crate::serde::tri;

impl serde::Serializer for Serializer {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = SerializeVec;
    type SerializeTuple = SerializeVec;
    type SerializeTupleStruct = SerializeVec;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeStructVariant;

    #[inline]
    fn serialize_unit(self) -> Result<Value> {
        Ok(Value::new_null())
    }

    #[inline]
    fn serialize_bool(self, value: bool) -> Result<Value> {
        Ok(Value::new_bool(value))
    }

    #[inline]
    fn serialize_i8(self, value: i8) -> Result<Value> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_i16(self, value: i16) -> Result<Value> {
        self.serialize_i64(value as i64)
    }

    #[inline]
    fn serialize_i32(self, value: i32) -> Result<Value> {
        self.serialize_i64(value as i64)
    }

    fn serialize_i64(self, value: i64) -> Result<Value> {
        Ok(Value::new_i64(value))
    }

    fn serialize_i128(self, value: i128) -> Result<Value> {
        if let Ok(value) = u64::try_from(value) {
            Ok(Value::new_u64(value))
        } else if let Ok(value) = i64::try_from(value) {
            Ok(Value::new_i64(value))
        } else {
            // FIXME: print i128 in error message
            Err(Error::ser_error(ErrorCode::NumberOutOfRange))
        }
    }

    #[inline]
    fn serialize_u8(self, value: u8) -> Result<Value> {
        self.serialize_u64(value as u64)
    }

    #[inline]
    fn serialize_u16(self, value: u16) -> Result<Value> {
        self.serialize_u64(value as u64)
    }

    #[inline]
    fn serialize_u32(self, value: u32) -> Result<Value> {
        self.serialize_u64(value as u64)
    }

    #[inline]
    fn serialize_u64(self, value: u64) -> Result<Value> {
        Ok(Value::new_u64(value))
    }

    fn serialize_u128(self, value: u128) -> Result<Value> {
        if let Ok(value) = u64::try_from(value) {
            Ok(Value::new_u64(value))
        } else {
            Err(Error::ser_error(ErrorCode::NumberOutOfRange))
        }
    }

    #[inline]
    fn serialize_f32(self, value: f32) -> Result<Value> {
        if value.is_finite() {
            Ok(unsafe { Value::new_f64_unchecked(value as f64) })
        } else {
            Ok(Value::new_null())
        }
    }

    #[inline]
    fn serialize_f64(self, value: f64) -> Result<Value> {
        if value.is_finite() {
            Ok(unsafe { Value::new_f64_unchecked(value) })
        } else {
            Ok(Value::new_null())
        }
    }

    #[inline]
    fn serialize_char(self, value: char) -> Result<Value> {
        Ok(Value::copy_str(&value.to_string()))
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<Value> {
        Ok(Value::copy_str(value))
    }

    // parse bytes as a array with u64
    fn serialize_bytes(self, value: &[u8]) -> Result<Value> {
        let mut array = Value::new_array_with(value.len());
        for b in value.iter() {
            array.append_value(Value::new_u64((*b) as u64));
        }
        Ok(array)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value> {
        self.serialize_str(variant)
    }

    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        let mut object = Value::new_object_with(1);
        object.insert(variant, tri!(to_value(value)));
        Ok(object)
    }

    #[inline]
    fn serialize_none(self) -> Result<Value> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(SerializeVec {
            vec: Value::new_array_with(len.unwrap_or_default()),
        })
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Ok(SerializeTupleVariant {
            static_name: variant,
            vec: Value::new_array_with(len),
        })
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(SerializeMap {
            map: MapInner::Object {
                object: Value::new_object_with(len.unwrap_or_default()),
                next_key: None,
            },
        })
    }

    #[inline]
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        match name {
            crate::serde::rawnumber::TOKEN => Ok(SerializeMap {
                map: MapInner::RawNumber { out_value: None },
            }),
            _ => self.serialize_map(Some(len)),
        }
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(SerializeStructVariant {
            static_name: variant,
            object: Value::new_object_with(len),
        })
    }

    #[inline]
    fn collect_str<T>(self, value: &T) -> Result<Value>
    where
        T: ?Sized + Display,
    {
        self.serialize_str(&value.to_string())
    }
}

/// Serializing Rust seq into `Value`.
pub(crate) struct SerializeVec {
    vec: Value,
}

/// Serializing Rust tuple variant into `Value`.
pub(crate) struct SerializeTupleVariant {
    static_name: &'static str,
    vec: Value,
}

/// Serializing Rust into `Value`. We has special handling for `Number`, `RawNumber`.
pub(crate) struct SerializeMap {
    map: MapInner,
}

enum MapInner {
    Object {
        object: Value,
        next_key: Option<Value>, // object key is value
    },
    RawNumber {
        out_value: Option<Value>,
    },
}

/// Serializing Rust struct variant into `Value`.
pub(crate) struct SerializeStructVariant {
    static_name: &'static str,
    object: Value,
}

impl serde::ser::SerializeSeq for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.append_value(tri!(to_value(value)));
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(self.vec)
    }
}

impl serde::ser::SerializeTuple for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl serde::ser::SerializeTupleStruct for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl serde::ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.append_value(tri!(to_value(value)));
        Ok(())
    }

    fn end(self) -> Result<Value> {
        let mut object = Value::new_object_with(1);
        object.insert(self.static_name, self.vec);
        Ok(object)
    }
}

impl serde::ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match &mut self.map {
            MapInner::Object { next_key, .. } => {
                *next_key = Some(tri!(key.serialize(MapKeySerializer)));
                Ok(())
            }
            MapInner::RawNumber { .. } => unreachable!(),
        }
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match &mut self.map {
            MapInner::Object { object, next_key } => {
                let key = next_key.take();
                // Panic because this indicates a bug in the program rather than an
                // expected failure.
                let key = key.expect("serialize_value called before serialize_key");
                object.insert(key.as_str().unwrap(), tri!(to_value(value)));
                Ok(())
            }
            MapInner::RawNumber { .. } => unreachable!(),
        }
    }

    fn end(self) -> Result<Value> {
        match self.map {
            MapInner::Object { object, .. } => Ok(object),
            MapInner::RawNumber { .. } => unreachable!(),
        }
    }
}

// Serialize the map key into a Value.
struct MapKeySerializer;

fn float_key_must_be_finite() -> Error {
    Error::ser_error(ErrorCode::FloatMustBeFinite)
}

impl serde::Serializer for MapKeySerializer {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = Impossible<Value, Error>;
    type SerializeTuple = Impossible<Value, Error>;
    type SerializeTupleStruct = Impossible<Value, Error>;
    type SerializeTupleVariant = Impossible<Value, Error>;
    type SerializeMap = Impossible<Value, Error>;
    type SerializeStruct = Impossible<Value, Error>;
    type SerializeStructVariant = Impossible<Value, Error>;

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value> {
        Ok(Value::from_static_str(variant))
    }

    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_bool(self, value: bool) -> Result<Value> {
        if value {
            Ok(Value::from_static_str("true"))
        } else {
            Ok(Value::from_static_str("false"))
        }
    }

    fn serialize_i8(self, value: i8) -> Result<Value> {
        self.serialize_i64(value as i64)
    }

    fn serialize_i16(self, value: i16) -> Result<Value> {
        self.serialize_i64(value as i64)
    }

    fn serialize_i32(self, value: i32) -> Result<Value> {
        self.serialize_i64(value as i64)
    }

    fn serialize_i64(self, value: i64) -> Result<Value> {
        self.serialize_str(itoa::Buffer::new().format(value))
    }

    fn serialize_u8(self, value: u8) -> Result<Value> {
        self.serialize_u64(value as u64)
    }

    fn serialize_u16(self, value: u16) -> Result<Value> {
        self.serialize_u64(value as u64)
    }

    fn serialize_u32(self, value: u32) -> Result<Value> {
        self.serialize_u64(value as u64)
    }

    // FIXME: optimize the copy overhead
    fn serialize_u64(self, value: u64) -> Result<Value> {
        self.serialize_str(itoa::Buffer::new().format(value))
    }

    fn serialize_f32(self, value: f32) -> Result<Value> {
        if value.is_finite() {
            self.serialize_str(ryu::Buffer::new().format_finite(value))
        } else {
            Err(float_key_must_be_finite())
        }
    }

    fn serialize_f64(self, value: f64) -> Result<Value> {
        if value.is_finite() {
            self.serialize_str(ryu::Buffer::new().format_finite(value))
        } else {
            Err(float_key_must_be_finite())
        }
    }

    #[inline]
    fn serialize_char(self, value: char) -> Result<Value> {
        self.serialize_str(&value.to_string())
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<Value> {
        Ok(Value::copy_str(value))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Value> {
        Err(key_must_be_str_or_num(Unexpected::Other("bytes")))
    }

    fn serialize_unit(self) -> Result<Value> {
        Err(key_must_be_str_or_num(Unexpected::Other("unit")))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value> {
        Err(key_must_be_str_or_num(Unexpected::Other("unit struct")))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        Err(key_must_be_str_or_num(Unexpected::Other("newtype variant")))
    }

    fn serialize_none(self) -> Result<Value> {
        Err(key_must_be_str_or_num(Unexpected::Other("none")))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        Err(key_must_be_str_or_num(Unexpected::Option))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(key_must_be_str_or_num(Unexpected::Seq))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(key_must_be_str_or_num(Unexpected::Other("tuple")))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(key_must_be_str_or_num(Unexpected::Other("tuple struct")))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(key_must_be_str_or_num(Unexpected::TupleVariant))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(key_must_be_str_or_num(Unexpected::Map))
    }

    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(key_must_be_str_or_num(Unexpected::Other(name)))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(key_must_be_str_or_num(Unexpected::StructVariant))
    }

    fn collect_str<T>(self, value: &T) -> Result<Value>
    where
        T: ?Sized + Display,
    {
        self.serialize_str(&value.to_string())
    }
}

struct RawNumberEmitter;

impl serde::ser::Serializer for RawNumberEmitter {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = Impossible<Value, Error>;
    type SerializeTuple = Impossible<Value, Error>;
    type SerializeTupleStruct = Impossible<Value, Error>;
    type SerializeTupleVariant = Impossible<Value, Error>;
    type SerializeMap = Impossible<Value, Error>;
    type SerializeStruct = Impossible<Value, Error>;
    type SerializeStructVariant = Impossible<Value, Error>;

    fn serialize_bool(self, _v: bool) -> Result<Value> {
        unreachable!()
    }

    fn serialize_i8(self, _v: i8) -> Result<Value> {
        unreachable!()
    }

    fn serialize_i16(self, _v: i16) -> Result<Value> {
        unreachable!()
    }

    fn serialize_i32(self, _v: i32) -> Result<Value> {
        unreachable!()
    }

    fn serialize_i64(self, _v: i64) -> Result<Value> {
        unreachable!()
    }

    fn serialize_u8(self, _v: u8) -> Result<Value> {
        unreachable!()
    }

    fn serialize_u16(self, _v: u16) -> Result<Value> {
        unreachable!()
    }

    fn serialize_u32(self, _v: u32) -> Result<Value> {
        unreachable!()
    }

    fn serialize_u64(self, _v: u64) -> Result<Value> {
        unreachable!()
    }

    fn serialize_f32(self, _v: f32) -> Result<Value> {
        unreachable!()
    }

    fn serialize_f64(self, _v: f64) -> Result<Value> {
        unreachable!()
    }

    fn serialize_char(self, _v: char) -> Result<Value> {
        unreachable!()
    }

    fn serialize_str(self, value: &str) -> Result<Value> {
        Ok(Value::new_rawnum(value))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Value> {
        unreachable!()
    }

    fn serialize_none(self) -> Result<Value> {
        unreachable!()
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        unreachable!()
    }

    fn serialize_unit(self) -> Result<Value> {
        unreachable!()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value> {
        unreachable!()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Value> {
        unreachable!()
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, _value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        unreachable!()
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        unreachable!()
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        unreachable!()
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        unreachable!()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        unreachable!()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        unreachable!()
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        unreachable!()
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        unreachable!()
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        unreachable!()
    }
}

impl serde::ser::SerializeStruct for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match &mut self.map {
            MapInner::Object { .. } => serde::ser::SerializeMap::serialize_entry(self, key, value),
            MapInner::RawNumber { out_value } => {
                if key == crate::serde::rawnumber::TOKEN {
                    *out_value = Some(tri!(value.serialize(RawNumberEmitter)));
                    Ok(())
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn end(self) -> Result<Value> {
        match self.map {
            MapInner::Object { .. } => serde::ser::SerializeMap::end(self),
            MapInner::RawNumber { out_value, .. } => {
                Ok(out_value.expect("number value was not emitted"))
            }
        }
    }
}

impl serde::ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.object.insert(key, tri!(to_value(value)));
        Ok(())
    }

    fn end(self) -> Result<Value> {
        let mut object = Value::new_object_with(1);
        object.insert(self.static_name, self.object);
        Ok(object)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::{to_value, Value};

    #[derive(Debug, serde::Serialize, Hash, Default, Eq, PartialEq)]
    struct User {
        string: String,
        number: i32,
        array: Vec<String>,
    }

    #[test]
    fn test_to_value() {
        use crate::{json, to_value, Value};

        let user = User {
            string: "hello".into(),
            number: 123,
            array: vec!["a".into(), "b".into(), "c".into()],
        };
        let got: Value = to_value(&user).unwrap();
        let expect = json!({
            "string": "hello",
            "number": 123,
            "array": ["a", "b", "c"],
        });
        assert_eq!(got, expect);

        let got: Value = to_value("hello").unwrap();
        assert_eq!(got, "hello");

        let got: Value = to_value(&123).unwrap();
        assert_eq!(got, 123);
    }

    #[test]
    fn test_ser_errors() {
        let mut map = HashMap::<User, i64>::new();
        map.insert(User::default(), 123);

        let got = to_value(&map);
        println!("{got:?}");
        assert!(got.is_err());
    }

    #[derive(Default, Clone, Serialize, Deserialize, Debug)]
    pub struct CommonArgs {
        pub app_name: Option<String>,
    }

    #[derive(Default, Clone, Serialize, Deserialize, Debug)]
    struct Foo {
        a: i64,
        b: Vec<Value>,
    }

    #[test]
    fn test_to_value2() {
        use crate::prelude::*;

        let mut value = Value::default();

        let args = CommonArgs {
            app_name: Some("test".to_string()),
        };
        let foo: Foo =
            crate::from_str(r#"{"a": 1, "b":[123, "a", {}, [], {"a":null}, ["b"], 1.23]}"#)
                .unwrap();

        value["arg"] = to_value(&args).unwrap_or_default();
        value["bool"] = to_value(&true).unwrap_or_default();
        value["foo"] = to_value(&foo).unwrap_or_default();
        value["arr"] = to_value(&[1, 2, 3]).unwrap_or_default();
        value["arr"][2] = to_value(&args).unwrap_or_default();

        assert_eq!(value["arr"][2]["app_name"].as_str(), Some("test"));
    }

    #[test]
    fn test_inf_or_nan_to_value() {
        assert_eq!(to_value(&f64::INFINITY).unwrap(), Value::new_null());
        assert_eq!(to_value(&f64::NAN).unwrap(), Value::new_null());
        assert_eq!(to_value(&f32::INFINITY).unwrap(), Value::new_null());
        assert_eq!(to_value(&f32::NAN).unwrap(), Value::new_null());
    }
}
