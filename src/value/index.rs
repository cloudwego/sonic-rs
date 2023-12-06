use super::{node::Value, value_trait::JsonValueMutTrait};
use crate::lazyvalue::LazyValue;
use crate::util::private::Sealed;
use crate::util::reborrow::DormantMutRef;
use crate::value::from::SharedCtxGuard;
use crate::value::object::DEFAULT_OBJ_CAP;
use crate::value::shared::Shared;
use crate::value::value_trait::JsonValueTrait;
use std::convert::Into;

impl<I> std::ops::Index<I> for Value
where
    I: Index,
{
    type Output = Value;

    /// Index into an array `Value` using the syntax `value[0]` and index into an
    /// object `Value` using the syntax `value["k"]`.
    ///
    /// Returns a null `Value` if the `Value` type does not match the index, or the
    /// index does not exist in the array or object.
    ///
    /// For retrieving deeply nested values, you should have a look at the `Value::pointer` method.
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
    /// assert_eq!(data["x"]["y"], json!(["z", "zz"]));
    /// assert_eq!(data["x"]["y"][0], json!("z"));
    ///
    /// assert_eq!(data["a"], json!(null)); // returns null for undefined values
    /// assert_eq!(data["a"]["b"], json!(null)); // does not panic
    ///
    /// // use pointer for retrieving nested values
    /// assert_eq!(data.pointer(&pointer!["x", "y", 0]).unwrap(), &json!("z"));
    /// ```

    #[inline]
    fn index(&self, index: I) -> &Value {
        static NULL: Value = Value::new();
        index.value_index_into(self).unwrap_or(&NULL)
    }
}

impl<I: Index> std::ops::IndexMut<I> for Value {
    /// Write the index of a mutable `Value`, and use the syntax `value[0] = ...`
    /// in an array and `value["k"] = ...` in an object.
    ///
    /// If the index is a number, the value must be an array of length bigger
    /// than the index. Indexing into a value that is not an array or an array
    /// that is too small will panic.
    ///
    /// If the index is a string, the value must be an object or null which is
    /// treated like an empty object. If the key is not already present in the
    /// object, it will be inserted with a value of null. Indexing into a value
    /// that is neither an object nor null will panic.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sonic_rs::json;
    /// #
    /// let mut data = json!({ "x": 0, "z": null });
    ///
    /// // replace an existing key
    /// data["x"] = json!(1);
    ///
    /// // insert a new key
    /// data["y"] = json!([1, 2, 3]);
    ///
    /// // replace an array value
    /// data["y"][0] = json!(true);
    ///
    /// // inserted a deeply nested key
    /// data["a"]["b"]["c"]["d"] = json!(true);
    ///
    /// //insert an key in a null value
    /// data["z"]["zz"] = json!("insert in null");
    ///
    /// assert_eq!(data, json!({
    ///   "x": 1,
    ///   "y": [true, 2, 3],
    ///   "a": { "b": {"c": {"d": true}}},
    ///    "z": {"zz": "insert in null"}
    /// }));
    ///
    /// ```
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Value {
        index.index_or_insert(self)
    }
}

/// An indexing trait for immutable `sonic_rs::Value`.
///
pub trait Index: Sealed {
    /// Return None if the index is not already in the array or object.
    #[doc(hidden)]
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value>;

    /// Return None if the index is not already in the array or object lazy_value.
    #[doc(hidden)]
    fn lazyvalue_index_into<'de>(&self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>>;

    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value>;

    /// Panic if array index out of bounds. If key is not already in the object,
    /// insert it with a value of null. Panic if Value is a type that cannot be
    /// indexed into, except if Value is null then it can be treated as an empty
    /// object.
    #[doc(hidden)]
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value;
}

impl Index for usize {
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        if !v.is_array() {
            return None;
        }
        v.get_index(*self)
    }

    fn lazyvalue_index_into<'de>(&self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>> {
        v.get_index(*self)
    }

    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        if !v.is_array() {
            return None;
        }
        v.get_index_mut(*self)
    }

    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        let typ = v.get_type();
        let len = v.len();
        v.as_array_mut()
            .unwrap_or_else(|| panic!("cannot access index in non-array value type {:?}", typ))
            .0
            .get_index_mut(*self)
            .unwrap_or_else(|| panic!("index {} out of bounds (len: {})", *self, len))
    }
}

macro_rules! impl_str_index {
    ($($t: ty),*) => {
        $(
            impl Index for &$t {
                fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
                    if !v.is_object() {
                        return None;
                    }
                    v.get_key(*self)
                }

                fn lazyvalue_index_into<'de>(&self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>> {
                    v.get_key(*self)
                }

                fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
                    if !v.is_object() {
                        return None;
                    }
                    v.get_key_mut(*self).map(|v| v.0)
                }

                fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
                    let mut shared = v.shared_parts();
                    if v.is_null() {
                        if shared.is_null() {
                            shared = Shared::new_ptr();
                            *v = Value::new_object(shared, 8);
                        } else {
                            unsafe { std::ptr::write(v, Value::new_object(shared, DEFAULT_OBJ_CAP)) };
                        }
                    }

                    let typ = v.get_type();
                    let (obj, mut dormant_obj) = DormantMutRef::new(v);
                    obj.as_object_mut()
                        .expect(&format!("cannot access key in non-object value {:?}", typ))
                        .0
                        .get_key_mut(*self).map_or_else(|| {
                            let o =  unsafe { dormant_obj.reborrow() };
                            let _ = SharedCtxGuard::assign(shared);
                            let inserted = o.append_pair((Into::<Value>::into((*self)), Value::new_null(shared)));
                            &mut inserted.1
                        }, |v| v.0)
                }
            }
        )*
    };
}

impl_str_index!(str, String, faststr::FastStr);

impl<T> Index for &T
where
    T: ?Sized + Index,
{
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        (**self).value_index_into(v)
    }

    fn lazyvalue_index_into<'de>(&self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>> {
        (**self).lazyvalue_index_into(v)
    }

    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        (**self).index_into_mut(v)
    }

    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        (**self).index_or_insert(v)
    }
}
