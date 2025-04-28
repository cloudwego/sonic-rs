use crate::{index::Index, JsonNumberTrait, Number, RawNumber};

/// JsonType is an enum that represents the type of a JSON value.
///
/// # Examples
///
/// ```
/// use sonic_rs::{JsonType, JsonValueTrait, Value};
///
/// let json: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
///
/// assert_eq!(json.get(&"a").unwrap().get_type(), JsonType::Number);
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
            _ => panic!("Invalid JsonType value from u8 {value}"),
        }
    }
}

/// A trait for all JSON values. Used by `Value` and `LazyValue`.
///
/// The `Option<V: JsonValueTrait>` and `Result<V: JsonValueTrait, E>` also implement this trait.
/// The `Option::None` or `Result::Err(_)` will be viewed as a null value.
pub trait JsonValueTrait {
    type ValueType<'v>
    where
        Self: 'v;

    /// Gets the type of the value. Returns `JsonType::Null` as default if `self` is `Option::None`
    /// or `Result::Err(_)`.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{
    ///     value::{JsonType, JsonValueTrait, Value},
    ///     Result,
    /// };
    ///
    /// let json: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
    ///
    /// assert_eq!(json.get_type(), JsonType::Object);
    ///
    /// let v: Option<&Value> = json.get("c");
    /// assert!(v.is_none());
    /// assert_eq!(v.get_type(), JsonType::Null);
    ///
    /// let v: Result<Value> = sonic_rs::from_str("invalid json");
    /// assert!(v.is_err());
    /// assert_eq!(v.get_type(), JsonType::Null);
    /// ```
    fn get_type(&self) -> JsonType;

    /// Returns true if the value is a `bool`.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// let val = json!(true);
    /// assert!(val.is_boolean());
    /// ```
    #[inline]
    fn is_boolean(&self) -> bool {
        self.get_type() == JsonType::Boolean
    }

    /// Returns true if the value is true.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// let val = json!(true);
    /// assert!(val.is_true());
    /// ```
    #[inline]
    fn is_true(&self) -> bool {
        self.as_bool().unwrap_or_default()
    }

    /// Returns true if the value is false.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// let val = json!(false);
    /// assert!(val.is_false());
    /// ```
    #[inline]
    fn is_false(&self) -> bool {
        !self.is_true()
    }

    /// Returns true if the `self` value is `null`.
    ///
    /// # Notes
    ///
    /// It will Returns true if `self` is `Option::None` or `Result::Err(_)`.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{json, JsonValueTrait, Result, Value};
    ///
    /// let val = json!(null);
    /// assert!(val.is_null());
    ///
    /// let val: Option<&Value> = val.get("unknown");
    /// assert!(val.is_none());
    /// assert!(val.is_null());
    ///
    /// let val: Result<Value> = sonic_rs::from_str("invalid json");
    /// assert!(val.is_err());
    /// assert!(val.is_null());
    /// ```
    #[inline]
    fn is_null(&self) -> bool {
        self.get_type() == JsonType::Null
    }

    /// Returns true if the value is a `number`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// assert!(json!(1).is_number());
    /// assert!(Option::Some(json!(1.23)).is_number());
    /// ```
    #[inline]
    fn is_number(&self) -> bool {
        self.get_type() == JsonType::Number
    }

    /// Returns true if the value is a `string`.
    ///  
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    /// assert!(json!("foo").is_str());
    /// ```
    #[inline]
    fn is_str(&self) -> bool {
        self.get_type() == JsonType::String
    }

    /// Returns true if the value is an `array`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    /// assert!(json!([]).is_array());
    /// ```
    #[inline]
    fn is_array(&self) -> bool {
        self.get_type() == JsonType::Array
    }

    /// Returns true if the value is an `object`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    /// assert!(json!({}).is_object());
    /// ```
    #[inline]
    fn is_object(&self) -> bool {
        self.get_type() == JsonType::Object
    }

    /// Returns true if the value is a number and it is an `f64`.
    /// It will returns false if the value is a `u64` or `i64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    /// assert!(!json!(123).is_f64()); // false
    /// assert!(!json!(-123).is_f64()); // false
    ///
    /// assert!(json!(-1.23).is_f64());
    /// ```
    #[inline]
    fn is_f64(&self) -> bool {
        self.as_number().map(|f| f.is_f64()).unwrap_or_default()
    }

    /// Returns true if the value is a integer number and it between `i64::MIN` and `i64::MAX`
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// assert!(json!(-123).is_i64());
    /// assert!(json!(0).is_i64());
    /// assert!(json!(123).is_i64());
    /// assert!(json!(i64::MIN).is_i64());
    /// assert!(json!(i64::MAX).is_i64());
    ///
    /// assert!(!json!(u64::MAX).is_i64()); // overflow for i64
    /// assert!(!json!(-1.23).is_i64()); // false
    /// ```
    #[inline]
    fn is_i64(&self) -> bool {
        self.as_i64().is_some()
    }

    /// Returns true if the value is a integer number and it between `0` and `i64::MAX`
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// assert!(json!(123).is_u64());
    /// assert!(json!(0).is_u64());
    /// assert!(json!(u64::MAX).is_u64());
    ///
    /// assert!(!json!(-123).is_u64());
    /// assert!(!json!(1.23).is_u64());
    /// ```
    #[inline]
    fn is_u64(&self) -> bool {
        self.as_u64().is_some()
    }

    /// If `self` meets `is_i64`, represent it as i64 if possible. Returns None otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// assert_eq!(json!(123).as_i64(), Some(123));
    /// assert_eq!(json!(-123).as_i64(), Some(-123));
    /// assert_eq!(json!(i64::MAX).as_i64(), Some(i64::MAX));
    ///
    /// assert_eq!(json!(u64::MAX).as_i64(), None);
    /// ```
    #[inline]
    fn as_i64(&self) -> Option<i64> {
        self.as_number().and_then(|n| n.as_i64())
    }

    /// If `self` meets `is_i64`, represent it as u64 if possible. Returns None otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// assert_eq!(json!(123).as_u64(), Some(123));
    /// assert_eq!(json!(-123).as_u64(), None);
    /// assert_eq!(json!(i64::MAX).as_u64(), Some(i64::MAX as u64));
    ///
    /// assert_eq!(json!(u64::MAX).as_u64(), Some(u64::MAX));
    /// ```
    #[inline]
    fn as_u64(&self) -> Option<u64> {
        self.as_number().and_then(|n| n.as_u64())
    }

    /// If `self` is a number, represent it as f64 if possible. Returns None otherwise.
    /// The integer number will be converted to f64.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    ///
    /// assert_eq!(json!(123).as_f64(), Some(123 as f64));
    /// assert_eq!(json!(-123).as_f64(), Some(-123 as f64));
    /// assert_eq!(json!(0.123).as_f64(), Some(0.123));
    ///
    /// assert_eq!(json!("hello").as_f64(), None);
    /// ```
    #[inline]
    fn as_f64(&self) -> Option<f64> {
        self.as_number().and_then(|n| n.as_f64())
    }

    /// Returns the `Number` if `self` is a `Number`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait, Number};
    ///
    /// assert_eq!(json!(123).as_number(), Some(Number::from(123)));
    /// ```
    fn as_number(&self) -> Option<Number>;

    /// Returns the [`RawNumber`] without precision loss if `self` is a `Number`.
    fn as_raw_number(&self) -> Option<RawNumber>;

    /// Returns the str if `self` is a `string`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    /// assert_eq!(json!("foo").as_str(), Some("foo"));
    /// ```
    fn as_str(&self) -> Option<&str>;

    /// Returns the bool if `self` is a `boolean`.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueTrait};
    /// assert_eq!(json!(true).as_bool(), Some(true));
    /// ```
    fn as_bool(&self) -> Option<bool>;

    /// Index into a JSON array or map. A string-like index can be used to access a
    /// value in a map, and a usize index can be used to access an element of an
    /// array.
    ///
    /// Returns `None` if the type of `self` does not match the type of the
    /// index, for example if the index is a string and `self` is an array or a
    /// number. Also returns `None` if the given key does not exist in the map
    /// or the given index is not within the bounds of the array.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::value::{JsonType, JsonValueTrait, Value};
    ///
    /// let json: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
    ///
    /// assert!(json.get("a").is_number());
    /// assert!(json.get("unknown").is_none());
    /// ```
    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>>;

    /// Looks up a value by a path.
    ///
    /// The path is an iterator of multiple keys or indexes. It can be a `&[&str]`, `&[usize]`
    /// or a `JsonPointer`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, pointer, JsonValueTrait};
    ///
    /// let data = json!({
    ///     "x": {
    ///         "y": ["z", "zz"]
    ///     }
    /// });
    ///
    /// assert_eq!(data.pointer(&["x", "y"] ).unwrap(), &json!(["z", "zz"]));
    /// assert_eq!(data.pointer(&pointer!["x", "y", 1] ).unwrap(), &json!("zz"));
    /// assert_eq!(data.pointer(&["a", "b"]), None);
    /// ```
    fn pointer<P: IntoIterator>(&self, path: P) -> Option<Self::ValueType<'_>>
    where
        P::Item: Index;
}

/// A trait for all JSON object or array values. Used by `Value`.
///
/// The `Option<V: JsonContainerTrait>` and `Result<V: JsonContainerTrait, E>` also implement this
/// trait. The `Option::None` or `Result::Err(_)` will be viewed as a null value.
pub trait JsonContainerTrait {
    type ObjectType;
    type ArrayType;

    /// Returns the object if `self` is an `object`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, object, JsonContainerTrait, JsonValueTrait, Value};
    ///
    /// let value: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
    ///
    /// assert!(value.is_object());
    /// assert_eq!(value.as_object(), Some(&object! {"a": 1, "b": true}));
    /// ```
    fn as_object(&self) -> Option<&Self::ObjectType>;

    /// Returns the array if `self` is an `array`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{array, json, JsonContainerTrait, JsonValueTrait, Value};
    ///
    /// let value: Value = sonic_rs::from_str(r#"[1, 2, 3]"#).unwrap();
    ///
    /// assert!(value.is_array());
    /// assert_eq!(value.as_array(), Some(&array![1, 2, 3]));
    /// ```
    fn as_array(&self) -> Option<&Self::ArrayType>;
}

/// A trait for all mutable JSON values. Used by mutable `Value`.
///
/// The `Option<V: JsonValueMutTrait>` and `Result<V: JsonValueMutTrait, E>` also implement this
/// trait. The `Option::None` or `Result::Err(_)` will be viewed as a null value.
pub trait JsonValueMutTrait {
    type ValueType;
    type ObjectType;
    type ArrayType;

    /// Returns the mutable object if `self` is an `object`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{JsonValueMutTrait, json};
    /// use sonic_rs::Value;
    ///
    /// let mut value: Value = sonic_rs::from_str(r#"{"a": 1, "b": true}"#).unwrap();
    /// let obj = value.as_object_mut().unwrap();
    /// obj["a"] = json!(2);
    /// assert_eq!(value, json!({"a": 2, "b": true}));
    /// ```
    fn as_object_mut(&mut self) -> Option<&mut Self::ObjectType>;

    /// Returns the mutable array if `self` is an `array`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueMutTrait, Value};
    ///
    /// let mut value: Value = sonic_rs::from_str(r#"[1, 2, 3]"#).unwrap();
    /// let arr = value.as_array_mut().unwrap();
    /// arr[0] = json!(2);
    /// assert_eq!(value, json!([2, 2, 3]));
    /// ```
    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType>;

    /// Looks up a value by a path.
    ///
    /// The path is an iterator of multiple keys or indexes. It can be a `&[&str]`, `&[usize]`
    /// or a `JsonPointer`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sonic_rs::{json, pointer, JsonValueTrait};
    /// #
    /// let data = json!({
    ///     "x": {
    ///         "y": ["z", "zz"]
    ///     }
    /// });
    ///
    /// assert_eq!(data.pointer(&["x", "y"] ).unwrap(), &json!(["z", "zz"]));
    /// assert_eq!(data.pointer(&pointer!["x", "y", 1] ).unwrap(), &json!("zz"));
    /// assert_eq!(data.pointer(&["a", "b"]), None);
    /// ```
    fn pointer_mut<P: IntoIterator>(&mut self, path: P) -> Option<&mut Self::ValueType>
    where
        P::Item: Index;

    /// Mutably index into a JSON array or map. A string-like index can be used to
    /// access a value in a map, and a usize index can be used to access an
    /// element of an array.
    ///
    /// Returns `None` if the type of `self` does not match the type of the
    /// index, for example if the index is a string and `self` is an array or a
    /// number. Also returns `None` if the given key does not exist in the map
    /// or the given index is not within the bounds of the array.
    ///
    /// ```
    /// use sonic_rs::{json, JsonValueMutTrait};
    ///
    /// let mut object = json!({ "A": 65, "B": 66, "C": 67 });
    /// *object.get_mut("A").unwrap() = json!(69);
    ///
    /// let mut array = json!([ "A", "B", "C" ]);
    /// *array.get_mut(2).unwrap() = json!("D");
    /// ```
    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType>;
}

impl<V: JsonValueTrait> JsonValueTrait for Option<V> {
    type ValueType<'v>
        = V::ValueType<'v>
    where
        V: 'v,
        Self: 'v;

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

    fn as_raw_number(&self) -> Option<RawNumber> {
        self.as_ref().and_then(|v| v.as_raw_number())
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

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<Self::ValueType<'_>>
    where
        P::Item: Index,
    {
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

    fn pointer_mut<P: IntoIterator>(&mut self, path: P) -> Option<&mut Self::ValueType>
    where
        P::Item: Index,
    {
        self.as_mut().and_then(|v| v.pointer_mut(path))
    }

    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType> {
        self.as_mut().and_then(|v| v.get_mut(index))
    }
}

impl<V: JsonValueTrait, E> JsonValueTrait for Result<V, E> {
    type ValueType<'v>
        = V::ValueType<'v>
    where
        V: 'v,
        Self: 'v;

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

    fn as_raw_number(&self) -> Option<RawNumber> {
        self.as_ref().ok().and_then(|v| v.as_raw_number())
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

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<Self::ValueType<'_>>
    where
        P::Item: Index,
    {
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

    fn pointer_mut<P: IntoIterator>(&mut self, path: P) -> Option<&mut Self::ValueType>
    where
        P::Item: Index,
    {
        self.as_mut().ok().and_then(|v| v.pointer_mut(path))
    }

    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType> {
        self.as_mut().ok().and_then(|v| v.get_mut(index))
    }
}

impl<V: JsonValueTrait> JsonValueTrait for &V {
    type ValueType<'v>
        = V::ValueType<'v>
    where
        V: 'v,
        Self: 'v;

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

    fn as_raw_number(&self) -> Option<RawNumber> {
        (*self).as_raw_number()
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

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<Self::ValueType<'_>>
    where
        P::Item: Index,
    {
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

    fn pointer_mut<P: IntoIterator>(&mut self, path: P) -> Option<&mut Self::ValueType>
    where
        P::Item: Index,
    {
        (**self).pointer_mut(path)
    }
}
