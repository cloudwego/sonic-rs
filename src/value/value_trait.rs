use crate::{value::Index, JsonNumberTrait, JsonPointer, Number};

/// JsonType is an enum that represents the type of a JSON value.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum JsonType {
    Null = 0,
    Boolean = 1,
    Number = 2,
    String = 3,
    Object = 4,
    Array = 5,
    Raw = 6,
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
            6 => JsonType::Raw,
            _ => panic!("invalid JsonType value: {}", value),
        }
    }
}

/// A trait for all JSON values. Used by `Value` and `LazyValue`.
pub trait JsonValue: Sized {
    type ValueType<'dom>
    where
        Self: 'dom;

    /// get the type of the `JsonValue`.
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
    /// The index may be usize or &str. The `usize` is for array, the `&str` is for object.
    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>>;

    /// Returns the value from pointer path if the `JsonValue` is an `array` or `object`
    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>>;
}

// A helper trait for Option types
impl<V: JsonValue> JsonValue for Option<V> {
    type ValueType<'dom> = V::ValueType<'dom>
        where
        V:'dom, Self: 'dom;

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

// A helper trait for Result types
impl<V: JsonValue, E> JsonValue for Result<V, E> {
    type ValueType<'dom> = V::ValueType<'dom>
        where
        V:'dom, Self: 'dom;

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

// A helper trait for reference types
impl<V: JsonValue> JsonValue for &V {
    type ValueType<'dom> = V::ValueType<'dom>
        where
        V:'dom, Self: 'dom;

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

// A helper trait for reference types
impl<V: JsonValue> JsonValue for &mut V {
    type ValueType<'dom> = V::ValueType<'dom>
        where
        V:'dom, Self: 'dom;

    fn as_bool(&self) -> Option<bool> {
        (**self).as_bool()
    }

    fn as_f64(&self) -> Option<f64> {
        (**self).as_f64()
    }

    fn as_i64(&self) -> Option<i64> {
        (**self).as_i64()
    }

    fn as_u64(&self) -> Option<u64> {
        (**self).as_u64()
    }

    fn as_number(&self) -> Option<Number> {
        (**self).as_number()
    }

    fn get_type(&self) -> JsonType {
        (**self).get_type()
    }

    fn as_str(&self) -> Option<&str> {
        (**self).as_str()
    }

    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        (**self).get(index)
    }

    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>> {
        (**self).pointer(path)
    }
}
