use super::index::Index;
use crate::{JsonNumberTrait, JsonPointer, Number};

/// JsonType is an enum that represents the type of a JSON value.
///
/// # Examples
/// ```
///  use sonic_rs::JsonType;
///  use sonic_rs::Value;
///
///  let json: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
///
///  assert_eq!(json.get("a").unwrap().get_type(), JsonType::Number);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum JsonType {
    Null = 0,
    Boolean = 1,
    Number = 2,
    String = 3,
    Object = 4,
    Array = 5,
}

impl From<u8> for JsonType {
    fn from(value: u8) -> Self {
        match value {
            0 => JsonType::Null,
            1 => JsonType::Boolean,
            2 => JsonType::Number,
            3 => JsonType::String,
            4 => JsonType::Object,
            5 => JsonType::Array,
            _ => panic!("invalid JsonType value: {}", value),
        }
    }
}

/// A trait for all JSON values. Used by `Value` and `LazyValue`.
pub trait JsonValueTrait {
    type ValueType<'v>
    where
        Self: 'v;

    /// Gets the type of the `JsonValue`. Returns `JsonType::Null` as default if `self` is `Option::None` or `Result::Err(_)`.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::value::JsonType;
    /// use sonic_rs::value::JsonValueTrait;
    /// use sonic_rs::value::Value;
    /// use sonic_rs::Result;
    ///
    /// let json: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
    ///
    /// assert_eq!(json.get_type(), JsonType::Object);
    ///
    /// let v: Option<&Value> = json.get("c");
    /// assert!(v.is_none());
    /// assert_eq!(v.get_type(), JsonType::Null);
    ///
    /// let v: Result<Value> =  sonic_rs::from_str(r#"{"invalid json"#);
    /// assert!(v.is_err());
    /// assert_eq!(v.get_type(), JsonType::Null);
    /// ```
    fn get_type(&self) -> JsonType;

    /// Returns true if the `JsonValue` is a `bool`.
    #[inline]
    fn is_boolean(&self) -> bool {
        self.get_type() == JsonType::Boolean
    }

    /// Returns true if the `JsonValue` is true.
    #[inline]
    fn is_true(&self) -> bool {
        self.as_bool().unwrap_or_default()
    }

    /// Returns true if the `JsonValue` is false.
    #[inline]
    fn is_false(&self) -> bool {
        !self.is_true()
    }

    /// Returns true if the `JsonValue` is `null`.
    #[inline]
    fn is_null(&self) -> bool {
        self.get_type() == JsonType::Null
    }

    /// Returns true if the `JsonValue` is a `number`.
    #[inline]
    fn is_number(&self) -> bool {
        self.get_type() == JsonType::Number
    }

    /// Returns true if the `JsonValue` is a `string`.
    #[inline]
    fn is_str(&self) -> bool {
        self.get_type() == JsonType::String
    }

    /// Returns true if the `JsonValue` is an `array`.
    #[inline]
    fn is_array(&self) -> bool {
        self.get_type() == JsonType::Array
    }

    /// Returns true if the `JsonValue` is an `object`.
    #[inline]
    fn is_object(&self) -> bool {
        self.get_type() == JsonType::Object
    }

    /// Returns true if the `JsonValue` is a `f64`.
    #[inline]
    fn is_f64(&self) -> bool {
        self.as_f64().is_some()
    }

    /// Returns true if the `JsonValue` is an `i64`.
    #[inline]
    fn is_i64(&self) -> bool {
        self.as_i64().is_some()
    }

    /// Returns true if the `JsonValue` is a `u64`.
    #[inline]
    fn is_u64(&self) -> bool {
        self.as_u64().is_some()
    }

    /// Returns the i64 value of the `JsonValue` if it is a `i64`.
    #[inline]
    fn as_i64(&self) -> Option<i64> {
        self.as_number().and_then(|n| n.as_i64())
    }

    /// Returns the u64 value of the `JsonValue` if it is a `u64`.
    #[inline]
    fn as_u64(&self) -> Option<u64> {
        self.as_number().and_then(|n| n.as_u64())
    }

    /// Returns the f64 value of the `JsonValue` if it is a `f64`.
    #[inline]
    fn as_f64(&self) -> Option<f64> {
        self.as_number().and_then(|n| n.as_f64())
    }

    /// Returns the `Number` value of the `JsonValue` if it is a `Number`.
    fn as_number(&self) -> Option<Number>;

    /// Returns the str if the `JsonValue` is a `string`.
    fn as_str(&self) -> Option<&str>;

    /// Returns the bool if the `JsonValue` is a `boolean`.
    fn as_bool(&self) -> Option<bool>;

    /// Returns the value from index if the `JsonValue` is an `array` or `object`
    /// The index may be usize or &str. The `usize` is for array, the `&str` is for object. Returns None otherwise.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::value::JsonType;
    /// use sonic_rs::value::JsonValueTrait;
    /// use sonic_rs::value::Value;
    ///
    /// let json: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
    ///
    /// assert!(json.get("a").is_number());
    /// assert!(json.get("unknown").is_none());
    /// ```
    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>>;

    /// Returns the value from pointer path if the `JsonValue` is an array or object. Returns None otherwise.
    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>>;
}

/// A trait for all JSON object or array values. Used by `Value`.
pub trait JsonContainerTrait {
    type ObjectType;
    type ArrayType;

    /// Returns the object if the `JsonValue` is an `object`.
    fn as_object(&self) -> Option<&Self::ObjectType>;

    /// Returns the array if the `JsonValue` is an `object`.
    fn as_array(&self) -> Option<&Self::ArrayType>;
}

/// A trait for all JSON values. Used by `Value` and `LazyValue`.
pub trait JsonValueMutTrait {
    type ValueType;
    type ObjectType;
    type ArrayType;

    /// Returns the mutable object if the `JsonValue` is an `object`.
    fn as_object_mut(&mut self) -> Option<&mut Self::ObjectType>;

    /// Returns the mutable array if the `JsonValue` is an `array`.
    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType>;

    /// Returns the value from pointer path if the `JsonValue` is an `array` or `object`
    fn pointer_mut(&mut self, path: &JsonPointer) -> Option<&mut Self::ValueType>;

    /// Returns the value from index if the `JsonValue` is an `array` or `object`
    /// The index may be usize or &str. The `usize` is for array, the `&str` is for object.
    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType>;
}

// A helper trait for Option types
impl<V: JsonValueTrait> JsonValueTrait for Option<V> {
    type ValueType<'v> = V::ValueType<'v> where V:'v, Self: 'v;

    fn as_bool(&self) -> Option<bool> {
        self.as_ref().and_then(|v| v.as_bool())
    }

    fn as_f64(&self) -> Option<f64> {
        self.as_ref().and_then(|v| v.as_f64())
    }

    fn as_i64(&self) -> Option<i64> {
        self.as_ref().and_then(|v| v.as_i64())
    }

    fn as_u64(&self) -> Option<u64> {
        self.as_ref().and_then(|v| v.as_u64())
    }

    fn as_number(&self) -> Option<Number> {
        self.as_ref().and_then(|v| v.as_number())
    }

    fn get_type(&self) -> JsonType {
        self.as_ref().map_or(JsonType::Null, |v| v.get_type())
    }

    fn as_str(&self) -> Option<&str> {
        self.as_ref().and_then(|v| v.as_str())
    }

    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        self.as_ref().and_then(|v| v.get(index))
    }

    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>> {
        self.as_ref().and_then(|v| v.pointer(path))
    }
}

impl<V: JsonContainerTrait> JsonContainerTrait for Option<V> {
    type ArrayType = V::ArrayType;
    type ObjectType = V::ObjectType;

    fn as_array(&self) -> Option<&Self::ArrayType> {
        self.as_ref().and_then(|v| v.as_array())
    }

    fn as_object(&self) -> Option<&Self::ObjectType> {
        self.as_ref().and_then(|v| v.as_object())
    }
}

impl<V: JsonValueMutTrait> JsonValueMutTrait for Option<V> {
    type ValueType = V::ValueType;
    type ArrayType = V::ArrayType;
    type ObjectType = V::ObjectType;

    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType> {
        self.as_mut().and_then(|v| v.as_array_mut())
    }
    fn as_object_mut(&mut self) -> Option<&mut Self::ObjectType> {
        self.as_mut().and_then(|v| v.as_object_mut())
    }

    fn pointer_mut(&mut self, path: &JsonPointer) -> Option<&mut Self::ValueType> {
        self.as_mut().and_then(|v| v.pointer_mut(path))
    }

    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType> {
        self.as_mut().and_then(|v| v.get_mut(index))
    }
}

// A helper trait for Result types
impl<V: JsonValueTrait, E> JsonValueTrait for Result<V, E> {
    type ValueType<'v> = V::ValueType<'v> where V:'v, Self: 'v;

    fn as_bool(&self) -> Option<bool> {
        self.as_ref().ok().and_then(|v| v.as_bool())
    }

    fn as_f64(&self) -> Option<f64> {
        self.as_ref().ok().and_then(|v| v.as_f64())
    }

    fn as_i64(&self) -> Option<i64> {
        self.as_ref().ok().and_then(|v| v.as_i64())
    }

    fn as_u64(&self) -> Option<u64> {
        self.as_ref().ok().and_then(|v| v.as_u64())
    }

    fn as_number(&self) -> Option<Number> {
        self.as_ref().ok().and_then(|v| v.as_number())
    }

    fn get_type(&self) -> JsonType {
        self.as_ref().ok().map_or(JsonType::Null, |v| v.get_type())
    }

    fn as_str(&self) -> Option<&str> {
        self.as_ref().ok().and_then(|v| v.as_str())
    }

    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        self.as_ref().ok().and_then(|v| v.get(index))
    }

    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>> {
        self.as_ref().ok().and_then(|v| v.pointer(path))
    }
}

impl<V: JsonContainerTrait, E> JsonContainerTrait for Result<V, E> {
    type ArrayType = V::ArrayType;
    type ObjectType = V::ObjectType;
    fn as_array(&self) -> Option<&Self::ArrayType> {
        self.as_ref().ok().and_then(|v| v.as_array())
    }

    fn as_object(&self) -> Option<&Self::ObjectType> {
        self.as_ref().ok().and_then(|v| v.as_object())
    }
}

impl<V: JsonValueMutTrait, E> JsonValueMutTrait for Result<V, E> {
    type ValueType = V::ValueType;
    type ArrayType = V::ArrayType;
    type ObjectType = V::ObjectType;

    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType> {
        self.as_mut().ok().and_then(|v| v.as_array_mut())
    }

    fn as_object_mut(&mut self) -> Option<&mut Self::ObjectType> {
        self.as_mut().ok().and_then(|v| v.as_object_mut())
    }

    fn pointer_mut(&mut self, path: &JsonPointer) -> Option<&mut Self::ValueType> {
        self.as_mut().ok().and_then(|v| v.pointer_mut(path))
    }

    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType> {
        self.as_mut().ok().and_then(|v| v.get_mut(index))
    }
}

impl<V: JsonValueTrait> JsonValueTrait for &V {
    type ValueType<'v> = V::ValueType<'v> where V:'v, Self: 'v;

    fn as_bool(&self) -> Option<bool> {
        (*self).as_bool()
    }

    fn as_f64(&self) -> Option<f64> {
        (*self).as_f64()
    }

    fn as_i64(&self) -> Option<i64> {
        (*self).as_i64()
    }

    fn as_u64(&self) -> Option<u64> {
        (*self).as_u64()
    }

    fn as_number(&self) -> Option<Number> {
        (*self).as_number()
    }

    fn get_type(&self) -> JsonType {
        (*self).get_type()
    }

    fn as_str(&self) -> Option<&str> {
        (*self).as_str()
    }

    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        (*self).get(index)
    }

    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>> {
        (*self).pointer(path)
    }
}

impl<V: JsonContainerTrait> JsonContainerTrait for &V {
    type ArrayType = V::ArrayType;
    type ObjectType = V::ObjectType;

    fn as_array(&self) -> Option<&Self::ArrayType> {
        (*self).as_array()
    }

    fn as_object(&self) -> Option<&Self::ObjectType> {
        (*self).as_object()
    }
}

impl<V: JsonValueMutTrait> JsonValueMutTrait for &mut V {
    type ValueType = V::ValueType;
    type ArrayType = V::ArrayType;
    type ObjectType = V::ObjectType;

    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType> {
        (*self).as_array_mut()
    }

    fn as_object_mut(&mut self) -> Option<&mut Self::ObjectType> {
        (*self).as_object_mut()
    }

    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType> {
        (**self).get_mut(index)
    }

    fn pointer_mut(&mut self, path: &JsonPointer) -> Option<&mut Self::ValueType> {
        (*self).pointer_mut(path)
    }
}
