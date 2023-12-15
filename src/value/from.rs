use super::array::{Array, DEFAULT_ARRAY_CAP};
use super::object::Object;
use crate::serde::number::N;
use crate::value::node::Value;
use crate::value::object::DEFAULT_OBJ_CAP;
use crate::value::shared::get_shared;
use crate::value::shared::get_shared_or_new;
use crate::value::shared::set_shared;
use crate::value::shared::Shared;
use crate::Number;
use faststr::FastStr;
use std::borrow::Cow;
use std::convert::Into;
use std::fmt::Debug;
use std::str::FromStr;

impl From<Number> for Value {
    /// Convert `Number` to a `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Number, Value, json};
    ///
    /// let x = Value::from(Number::from(7));
    /// assert_eq!(x, json!(7));
    /// ```
    #[inline]
    fn from(val: Number) -> Self {
        let shared = get_shared();
        match val.n {
            N::PosInt(u) => Value::new_u64(u, shared),
            N::NegInt(i) => Value::new_i64(i, shared),
            N::Float(f) => unsafe { Value::new_f64_unchecked(f, shared) },
        }
    }
}

macro_rules! impl_from_integer {
    ($($ty:ident),*) => {
        $(
            impl From<$ty> for Value {
                fn from(val: $ty) -> Self {
                    Into::<Number>::into(val).into()
                }
            }
        )*
    };
    () => {};
}

impl_from_integer!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl From<bool> for Value {
    /// Convert `bool` to a boolean `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::Value;
    /// use sonic_rs::JsonValueTrait;
    ///
    /// let x: Value = true.into();
    /// assert!(x.is_true());
    /// ```
    #[inline]
    fn from(val: bool) -> Self {
        Value::new_bool(val, get_shared())
    }
}

macro_rules! impl_from_str {
    () => {};
    ($($ty:ident),*) => {
        $(
            impl From<&$ty> for Value {
                /// Convert a string type into a string `Value`. The string will be copied into the `Value`.
                ///
                /// # Performance
                ///
                /// If it is `&'static str`, recommend to use [`Value::from_static_str`] and it is zero-copy.
                ///
                #[inline]
                fn from(val: &$ty) -> Self {
                    let (shared, is_root) = get_shared_or_new();
                    let mut value = Value::copy_str(val, shared);
                    if is_root {
                        value.mark_root();
                    }
                    value
                }
            }
        )*
    };
}

impl_from_str!(String, str, FastStr);

impl<'a> From<Cow<'a, str>> for Value {
    /// Convert copy-on-write string to a string `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::Value;
    /// use std::borrow::Cow;
    ///
    /// let s1: Cow<str> = Cow::Borrowed("hello");
    /// let x1 = Value::from(s1);
    ///
    /// let s2: Cow<str> = Cow::Owned("hello".to_string());
    /// let x2 = Value::from(s2);
    ///
    /// assert_eq!(x1, x2);
    /// ```
    #[inline]
    fn from(value: Cow<'a, str>) -> Self {
        Into::<Self>::into(value.as_ref())
    }
}

impl From<char> for Value {
    /// Convert `char` to a string `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, json};
    ///
    /// let c: char = '😁';
    /// let x: Value = c.into();
    /// assert_eq!(x, json!("😁"));
    /// ```
    #[inline]
    fn from(val: char) -> Self {
        Into::<Self>::into(&val.to_string())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    /// Convert a `Vec` to a `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, json};
    ///
    /// assert_eq!(Value::from(vec!["hi", "hello"]), json!(["hi", "hello"]));
    ///
    /// assert_eq!(Value::from(Vec::<i32>::new()), json!([]));
    ///
    /// assert_eq!(Value::from(vec![json!(null), json!("hi")]), json!([null, "hi"]));
    ///
    /// ```
    #[inline]
    fn from(val: Vec<T>) -> Self {
        let shared = get_shared();
        let is_root = shared.is_null();
        if val.is_empty() {
            return Value::new_array(shared, 0);
        }

        let mut array = if is_root {
            let new_shared = Shared::new_ptr();
            set_shared(new_shared);
            Value::new_array(new_shared, val.len())
        } else {
            Value::new_array(shared, val.len())
        };

        for v in val {
            // new create value will use the shared allocator.
            array.append_value(Into::<Value>::into(v));
        }
        if is_root {
            set_shared(std::ptr::null());
            array.mark_root();
        }
        array
    }
}

impl<T: Clone + Into<Value>, const N: usize> From<&[T; N]> for Value {
    /// Convert a array reference `&[T; N]` to a `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, json};
    ///
    /// let x  = Value::from(&["hi", "hello"]);
    ///
    /// assert_eq!(x, json!(["hi", "hello"]));
    ///
    #[inline]
    fn from(val: &[T; N]) -> Self {
        Into::<Value>::into(val.as_ref())
    }
}

impl<T: Clone + Into<Value>> From<&[T]> for Value {
    /// Convert a slice `&[T]` to a `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, json};
    ///
    /// let x  = Value::from(&["hi", "hello"][..]);
    ///
    /// assert_eq!(x, json!(["hi", "hello"]));
    ///
    /// let x: &[i32] = &[];
    /// assert_eq!(Value::from(x), json!([]));
    /// ```
    fn from(val: &[T]) -> Self {
        let shared = get_shared();
        let is_root = shared.is_null();
        if val.is_empty() {
            return Value::new_array(shared, 0);
        }

        let mut array = if is_root {
            let new_shared = Shared::new_ptr();
            set_shared(new_shared);
            Value::new_array(new_shared, val.len())
        } else {
            Value::new_array(shared, val.len())
        };
        for v in val {
            // new create value will use the shared allocator.
            array.append_value(Into::<Value>::into(v.clone()));
        }

        if is_root {
            array.mark_root();
            set_shared(std::ptr::null());
        }
        array
    }
}

impl From<()> for Value {
    /// Convert `()` to `Value::Null`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::Value;
    /// use sonic_rs::JsonValueTrait;
    ///
    /// assert!(Value::from(()).is_null());
    ///
    /// ```
    #[inline]
    fn from(_: ()) -> Self {
        let shared = get_shared();
        Value::new_null(shared)
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    /// Convert `Option` to `Value::Null`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::Value;
    /// use sonic_rs::JsonValueTrait;
    ///
    /// let u = Some(123);
    /// let x = Value::from(u);
    /// assert_eq!(x.as_i64(), u);
    ///
    /// let u = None;
    /// let x: Value = u.into();
    /// assert_eq!(x.as_i64(), u);
    /// ```
    #[inline]
    fn from(opt: Option<T>) -> Self {
        match opt {
            None => Into::into(()),
            Some(value) => Into::into(value),
        }
    }
}

impl FromStr for Value {
    type Err = crate::Error;
    /// Convert `&str` to `Value`. The `&str` will be copied into the `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::Value;
    /// use sonic_rs::JsonValueTrait;
    /// use std::str::FromStr;
    ///
    /// let x = Value::from_str("string").unwrap();
    /// assert_eq!(x.as_str().unwrap(), "string");
    /// ```
    /// # Performance
    ///
    /// If it is `&'static str`, recommend to use [`Value::from_static_str`].
    ///
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Value::new_str_owned(s))
    }
}

impl<'a, K: AsRef<str>, V: Clone + Into<Value>> FromIterator<(K, &'a V)> for Value {
    /// Create a `Value` by collecting an iterator of key-value pairs.
    /// The key will be copied into the `Value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, json, object};
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("sonic_rs", 40);
    /// map.insert("json", 2);
    ///
    /// let x: Value = map.iter().collect();
    /// assert_eq!(x, json!({"sonic_rs": 40, "json": 2}));
    ///
    /// let x: Value = Value::from_iter(&object!{"sonic_rs": 40, "json": 2});
    /// assert_eq!(x, json!({"sonic_rs": 40, "json": 2}));
    /// ```
    ///
    fn from_iter<T: IntoIterator<Item = (K, &'a V)>>(iter: T) -> Self {
        let (shared, is_root) = get_shared_or_new();
        if is_root {
            set_shared(shared);
        }

        let mut obj = Value::new_object(shared, DEFAULT_OBJ_CAP);
        for (k, v) in iter.into_iter() {
            let k = Value::copy_str(k.as_ref(), shared);
            // will create value use `shared` allocator
            let v = v.clone().into();
            obj.append_pair((k, v));
        }

        if is_root {
            obj.mark_root();
            set_shared(std::ptr::null());
        }
        obj
    }
}

impl<T: Into<Value>> FromIterator<T> for Value {
    /// Create a `Value` by collecting an iterator of array elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, json};
    /// use std::iter::FromIterator;
    ///
    /// let v = std::iter::repeat(6).take(3);
    /// let x: Value = v.collect();
    /// assert_eq!(x, json!([6, 6, 6]));
    ///
    /// let x = Value::from_iter(vec!["sonic_rs", "json", "serde"]);
    /// assert_eq!(x, json!(["sonic_rs", "json", "serde"]));
    /// ```
    ///
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let (shared, is_root) = get_shared_or_new();
        if is_root {
            set_shared(shared);
        }

        let mut arr = Value::new_array(shared, DEFAULT_ARRAY_CAP);
        for v in iter.into_iter() {
            // will create value use `shared` allocator
            arr.append_value(v.into());
        }

        if is_root {
            arr.mark_root();
            set_shared(std::ptr::null());
        }
        arr
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<T: Into<Value>> From<Vec<T>> for Array {
    /// Convert a `Vec` to a `Array`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::value::Array;
    /// use sonic_rs::array;
    ///
    /// let v = vec!["hi", "hello"];
    /// let x: Array = v.into();
    /// assert_eq!(x, array!["hi", "hello"]);
    /// ```
    #[inline]
    fn from(val: Vec<T>) -> Self {
        debug_assert!(get_shared().is_null(), "array should not be shared");
        let value = Into::<Value>::into(val);
        Array(value)
    }
}

impl<T: Clone + Into<Value>, const N: usize> From<&[T; N]> for Array {
    /// Convert a array `&[T; N]` to a `Array`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Array, array};
    ///
    /// let v = &["hi", "hello"];
    /// let x: Array = v.into();
    /// assert_eq!(x, array!["hi", "hello"]);
    /// ```
    ///
    fn from(val: &[T; N]) -> Self {
        debug_assert!(get_shared().is_null(), "array should not be shared");
        let value = Into::<Value>::into(val.as_ref());
        Array(value)
    }
}

impl<T: Into<Value>> FromIterator<T> for Array {
    /// Create a `Array` by collecting an iterator of array elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Array, json, array};
    /// use std::iter::FromIterator;
    ///
    /// let v = std::iter::repeat(6).take(3);
    /// let x: Array = v.collect();
    /// assert_eq!(x, json!([6, 6, 6]));
    ///
    /// let x = Array::from_iter(vec!["sonic_rs", "json", "serde"]);
    /// assert_eq!(x, array!["sonic_rs", "json", "serde"]);
    /// ```
    ///
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        debug_assert!(get_shared().is_null(), "array should not be shared");
        let value = Value::from_iter(iter);
        Array(value)
    }
}

impl<T: Clone + Into<Value>> From<&[T]> for Array {
    /// Convert a slice `&[T]` to a `Array`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::value::Array;
    /// use sonic_rs::array;
    ///
    /// let v = &["hi", "hello"];
    /// let x: Array = v.into();
    /// assert_eq!(x, array!["hi", "hello"]);
    /// ```
    ///
    fn from(val: &[T]) -> Self {
        debug_assert!(get_shared().is_null(), "array should not be shared");
        let value = Into::<Value>::into(val);
        Array(value)
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a, K: AsRef<str>, V: Clone + Into<Value> + 'a> FromIterator<(K, &'a V)> for Object {
    /// Create a `Object` by collecting an iterator of key-value pairs.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Value, object, Object};
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("sonic_rs", 40);
    /// map.insert("json", 2);
    ///
    /// let x: Object = map.iter().collect();
    /// assert_eq!(x, object!{"sonic_rs": 40, "json": 2});
    ///
    /// let x = Object::from_iter(&object!{"sonic_rs": 40, "json": 2});
    /// assert_eq!(x, object!{"sonic_rs": 40, "json": 2});
    /// ```
    ///
    #[inline]
    fn from_iter<T: IntoIterator<Item = (K, &'a V)>>(iter: T) -> Self {
        debug_assert!(get_shared().is_null(), "object should not be shared");
        let value = Value::from_iter(iter);
        Object(value)
    }
}

impl<'a, T: Clone + Into<Value> + 'a> Extend<&'a T> for Array {
    /// Extend a `Array` with the contents of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Array, array, json};
    /// let mut arr = array![];
    ///
    /// // array extend from a slice &[i32]
    /// arr.extend(&[1, 2, 3]);
    /// assert_eq!(arr, array![1, 2, 3]);
    ///
    /// arr.extend(&Array::default());
    /// assert_eq!(arr, array![1, 2, 3]);
    ///
    /// // array extend from other array
    /// arr.extend(&array![4, 5, 6]);
    /// assert_eq!(arr, array![1, 2, 3, 4, 5, 6]);
    ///
    /// ```
    ///
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        debug_assert!(
            get_shared().is_null(),
            "array extend should not use outer shared allocator"
        );
        let shared = self.0.check_shared();
        set_shared(shared);
        for v in iter {
            // new create value will use `shared` allocator
            self.push(v.clone().into());
        }
        set_shared(std::ptr::null());
    }
}

impl<'a, K: AsRef<str> + ?Sized, V: Clone + Debug + Into<Value> + 'a> Extend<(&'a K, &'a V)>
    for Object
{
    /// Extend a `Object` with the contents of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{Object, object, json, Value};
    /// use std::collections::HashMap;
    ///
    /// let mut obj = object![];
    /// let mut map: HashMap<&str, Value> ={
    ///     let mut map = HashMap::new();
    ///     map.insert("sonic", json!(40));
    ///     map.insert("rs", json!(null));
    ///     map
    /// };
    ///
    /// obj.extend(&map);
    /// assert_eq!(obj, object!{"sonic": 40, "rs": null});
    ///
    /// obj.extend(&object!{"object": [1, 2, 3]});
    /// assert_eq!(obj, object!{"sonic": 40, "rs": null, "object": [1, 2, 3]});
    ///
    /// ```
    ///
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: I) {
        debug_assert!(
            get_shared().is_null(),
            "object extend should not use outer shared allocator"
        );
        let shared = self.0.check_shared() as *const _;
        set_shared(shared);
        for (k, v) in iter {
            let k = Value::copy_str(k.as_ref(), self.0.shared());
            // new create value will use `shared` allocator
            let mut v = v.clone().into();
            v.unmark_root();
            self.0.append_pair((k, v));
        }
        set_shared(std::ptr::null());
    }
}

impl From<Array> for Value {
    #[inline]
    fn from(val: Array) -> Self {
        val.0
    }
}

impl From<Object> for Value {
    #[inline]
    fn from(val: Object) -> Self {
        val.0
    }
}

#[cfg(test)]
mod test {

    use crate::array;
    use crate::json;
    use crate::object;
    use crate::value::node::Value;
    use std::collections::HashMap;

    #[test]
    fn test_from() {
        let a1 = json!([1, 2, 3]);
        let a2: Value = vec![1, 2, 3].into();
        assert_eq!(a1, a2);
        let v = Value::from(vec![json!("hi")]);
        dbg!(&v);
        assert_eq!(v, json!(["hi"]));
    }

    #[test]
    fn test_extend_array() {
        let mut a1 = array![1, 2, 3];
        let mut b1 = a1.clone();

        let a2 = vec![4, 5, 6];
        let a3 = array![4, 5, 6];
        a1.extend(&a2);
        b1.extend(&a3);
        assert_eq!(a1, b1);
    }

    #[test]
    fn test_extend_object() {
        let mut obj = object![];
        let mut map: HashMap<&str, Value> = HashMap::new();

        map.insert("sonic_rs", json!(40));
        map.insert("json", "hi".into());
        obj.extend(map.iter());
    }

    #[test]
    fn test_from_iter() {
        use crate::{json, Value};
        use std::collections::HashMap;
        use std::iter::FromIterator;

        let mut map = HashMap::new();
        map.insert("sonic_rs", 40);
        map.insert("json", 2);

        let x: Value = map.iter().collect();
        assert_eq!(x, json!({"sonic_rs": 40, "json": 2}));

        let v = std::iter::repeat(6).take(3);
        let x1: Vec<_> = v.collect();
        dbg!(x1);
        let v = std::iter::repeat(6).take(3);
        let x: Value = v.collect();
        assert_eq!(x, json!([6, 6, 6]));

        let x = Value::from_iter(vec!["sonic_rs", "json", "serde"]);
        assert_eq!(x, json!(["sonic_rs", "json", "serde"]));
    }
}
