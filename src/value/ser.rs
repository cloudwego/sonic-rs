use core::fmt::Display;
use std::ptr::NonNull;

use serde::{
    de::Unexpected,
    ser::{Impossible, Serialize},
};

use super::shared::Shared;
use crate::{
    error::{Error, ErrorCode, Result},
    serde::ser::key_must_be_str_or_num,
    util::arc::Arc,
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
    let shared = Shared::new_ptr();
    let mut value = to_value_in(unsafe { NonNull::new_unchecked(shared as *mut _) }, value)?;
    if value.is_scalar() {
        value.mark_shared(std::ptr::null());
        std::mem::drop(unsafe { Arc::from_raw(shared) });
    } else {
        value.mark_root();
    }
    Ok(value)
}

// Not export this because it is mainly used in `json!`.
pub(crate) struct Serializer(NonNull<Shared>);

impl Serializer {
    #[inline]
    fn new_in(share: NonNull<Shared>) -> Self {
        Self(share)
    }

    #[inline]
    fn shared(&self) -> NonNull<Shared> {
        self.0
    }

    #[inline]
    fn shared_ptr(&self) -> *const Shared {
        self.0.as_ptr()
    }

    #[inline]
    fn shared_ref(&self) -> &Shared {
        unsafe { self.0.as_ref() }
    }
}

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
        Ok(Value::new_null(self.shared_ptr()))
    }

    #[inline]
    fn serialize_bool(self, value: bool) -> Result<Value> {
        Ok(Value::new_bool(value, self.shared_ptr()))
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
        Ok(Value::new_i64(value, self.shared_ptr()))
    }

    fn serialize_i128(self, value: i128) -> Result<Value> {
        if let Ok(value) = u64::try_from(value) {
            Ok(Value::new_u64(value, self.shared_ptr()))
        } else if let Ok(value) = i64::try_from(value) {
            Ok(Value::new_i64(value, self.shared_ptr()))
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
        Ok(Value::new_u64(value, self.shared_ptr()))
    }

    fn serialize_u128(self, value: u128) -> Result<Value> {
        if let Ok(value) = u64::try_from(value) {
            Ok(Value::new_u64(value, self.shared_ptr()))
        } else {
            Err(Error::ser_error(ErrorCode::NumberOutOfRange))
        }
    }

    #[inline]
    fn serialize_f32(self, value: f32) -> Result<Value> {
        if value.is_finite() {
            Ok(unsafe { Value::new_f64_unchecked(value as f64, self.shared_ptr()) })
        } else {
            Err(key_must_be_str_or_num(Unexpected::Other(
                "NaN or Infinite f32",
            )))
        }
    }

    #[inline]
    fn serialize_f64(self, value: f64) -> Result<Value> {
        if value.is_finite() {
            Ok(unsafe { Value::new_f64_unchecked(value, self.shared_ptr()) })
        } else {
            Err(key_must_be_str_or_num(Unexpected::Other(
                "NaN or Infinite f64",
            )))
        }
    }

    #[inline]
    fn serialize_char(self, value: char) -> Result<Value> {
        Ok(Value::copy_str(&value.to_string(), self.shared_ref()))
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<Value> {
        Ok(Value::copy_str(value, self.shared_ref()))
    }

    // parse bytes as a array with u64
    fn serialize_bytes(self, value: &[u8]) -> Result<Value> {
        let mut array = Value::new_array(self.shared_ptr(), value.len());
        for b in value.iter() {
            array.append_value(Value::new_u64((*b) as u64, self.shared_ptr()));
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
        let mut object = Value::new_object(self.shared_ptr(), 1);
        let pair = (
            Value::new_str(variant, self.shared_ptr()),
            tri!(to_value_in(self.shared(), value)),
        );
        object.append_pair(pair);
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
            shared: self.shared(),
            vec: Value::new_array(self.shared_ptr(), len.unwrap_or_default()),
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
            shared: self.shared(),
            static_name: Value::new_str(variant, self.shared_ptr()),
            vec: Value::new_array(self.shared_ptr(), len),
        })
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(SerializeMap {
            map: MapInner::Object {
                object: Value::new_object(self.shared_ptr(), len.unwrap_or_default()),
                next_key: None,
            },
            shared: self.shared(),
        })
    }

    #[inline]
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        match name {
            crate::serde::rawnumber::TOKEN => Ok(SerializeMap {
                map: MapInner::RawNumber { out_value: None },
                shared: self.shared(),
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
            shared: self.shared(),
            static_name: Value::new_str(variant, self.shared_ptr()),
            object: Value::new_object(self.shared_ptr(), len),
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
    shared: NonNull<Shared>,
    vec: Value,
}

/// Serializing Rust tuple variant into `Value`.
pub(crate) struct SerializeTupleVariant {
    shared: NonNull<Shared>,
    static_name: Value,
    vec: Value,
}

/// Serializing Rust into `Value`. We has special handling for `Number`, `RawNumber` and `RawValue`.
pub(crate) struct SerializeMap {
    map: MapInner,
    shared: NonNull<Shared>,
}

enum MapInner {
    Object {
        object: Value,
        next_key: Option<Value>, // object key is value
    },
    RawNumber {
        out_value: Option<Value>,
    },
    RawValue {
        out_value: Option<Value>,
    },
}

/// Serializing Rust struct variant into `Value`.
pub(crate) struct SerializeStructVariant {
    static_name: Value,
    object: Value,
    shared: NonNull<Shared>,
}

impl serde::ser::SerializeSeq for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.append_value(tri!(to_value_in(self.shared, value)));
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
        self.vec.append_value(tri!(to_value_in(self.shared, value)));
        Ok(())
    }

    fn end(self) -> Result<Value> {
        let mut object = Value::new_object(self.shared.as_ptr(), 1);
        object.append_pair((self.static_name, self.vec));
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
                *next_key = Some(tri!(key.serialize(MapKeySerializer(self.shared))));
                Ok(())
            }
            MapInner::RawNumber { .. } => unreachable!(),
            MapInner::RawValue { .. } => unreachable!(),
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
                object.append_pair((key, tri!(to_value_in(self.shared, value))));
                Ok(())
            }
            MapInner::RawNumber { .. } => unreachable!(),
            MapInner::RawValue { .. } => unreachable!(),
        }
    }

    fn end(self) -> Result<Value> {
        match self.map {
            MapInner::Object { object, .. } => Ok(object),
            MapInner::RawNumber { .. } => unreachable!(),
            MapInner::RawValue { .. } => unreachable!(),
        }
    }
}

// Serialize the map key into a Value.
struct MapKeySerializer(NonNull<Shared>);

impl MapKeySerializer {
    fn shared_ptr(&self) -> *const Shared {
        self.0.as_ptr()
    }
}

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
        Ok(Value::new_str(variant, self.shared_ptr()))
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
            Ok(Value::new_str("true", self.shared_ptr()))
        } else {
            Ok(Value::new_str("false", self.shared_ptr()))
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
        let shared = unsafe { self.0.as_ref() };
        Ok(Value::copy_str(value, shared))
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

impl serde::ser::SerializeStruct for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match &mut self.map {
            MapInner::Object { .. } => serde::ser::SerializeMap::serialize_entry(self, key, value),
            MapInner::RawNumber { out_value: _ } => {
                todo!()
            }
            MapInner::RawValue { out_value: _ } => {
                todo!()
            }
        }
    }

    fn end(self) -> Result<Value> {
        match self.map {
            MapInner::Object { .. } => serde::ser::SerializeMap::end(self),
            MapInner::RawNumber { out_value, .. } => {
                Ok(out_value.expect("number value was not emitted"))
            }
            MapInner::RawValue { out_value, .. } => {
                Ok(out_value.expect("raw value was not emitted"))
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
        self.object.append_pair((
            Value::new_str(key, self.shared.as_ptr()),
            tri!(to_value_in(self.shared, value)),
        ));
        Ok(())
    }

    fn end(self) -> Result<Value> {
        let mut object = Value::new_object(self.shared.as_ptr(), 1);
        object.append_pair((self.static_name, self.object));
        Ok(object)
    }
}

#[doc(hidden)]
#[inline]
pub fn to_value_in<T>(shared: NonNull<Shared>, value: &T) -> Result<Value>
where
    T: ?Sized + Serialize,
{
    let serializer = Serializer::new_in(shared);
    value.serialize(serializer)
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
        println!("{:?}", got);
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
    #[cfg(not(feature = "arbitrary_precision"))]
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
}
