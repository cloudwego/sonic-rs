use crate::{
    util::{private::Sealed, reborrow::DormantMutRef},
    JsonValueMutTrait, JsonValueTrait, PointerNode, Value,
};

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
    /// # use sonic_rs::object;
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
    /// data["z"]["zz1"] = object!{}.into();
    ///
    ///
    /// assert_eq!(data, json!({
    ///   "x": 1,
    ///   "y": [true, 2, 3],
    ///   "a": { "b": {"c": {"d": true}}},
    ///    "z": {"zz": "insert in null", "zz1": {}}
    /// }));
    /// ```
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Value {
        index.index_or_insert(self)
    }
}

/// An indexing trait for JSON.
pub trait Index: Sealed {
    /// Return None if the index is not already in the array or object.
    #[doc(hidden)]
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value>;

    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value>;

    /// Panic if array index out of bounds. If key is not already in the object,
    /// insert it with a value of null. Panic if Value is a type that cannot be
    /// indexed into, except if Value is null then it can be treated as an empty
    /// object.
    #[doc(hidden)]
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value;

    #[doc(hidden)]
    fn as_key(&self) -> Option<&str> {
        None
    }

    #[doc(hidden)]
    fn as_index(&self) -> Option<usize> {
        None
    }
}

impl Index for usize {
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        if !v.is_array() {
            return None;
        }
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
            .unwrap_or_else(|| panic!("cannot access index in non-array value type {typ:?}"))
            .0
            .get_index_mut(*self)
            .unwrap_or_else(|| panic!("index {} out of bounds (len: {})", *self, len))
    }

    fn as_index(&self) -> Option<usize> {
        Some(*self)
    }
}

macro_rules! impl_str_index {
    ($($t: ty),*) => {
        $(
            impl Index for &$t {
                #[inline]
                fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
                    if !v.is_object() {
                        return None;
                    }
                    v.get_key(*self)
                }

                #[inline]
                fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
                    if !v.is_object() {
                        return None;
                    }
                    v.get_key_mut(*self)
                }

                #[inline]
                fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
                    if v.is_null() {
                        *v = Value::new_object_with(8);
                    }

                    let typ = v.get_type();
                    let (obj, mut dormant_obj) = DormantMutRef::new(v);
                    obj.as_object_mut()
                        .expect(&format!("cannot access key in non-object value {:?}", typ))
                        .0
                        .get_key_mut(*self).unwrap_or_else(|| {
                            let o =  unsafe { dormant_obj.reborrow() };
                            let inserted = o.insert(&self, Value::new_null());
                            inserted
                        })
                }

                #[inline]
                fn as_key(&self) -> Option<&str> {
                    Some(self.as_ref())
                }
            }
        )*
    };
}

impl_str_index!(str, String, faststr::FastStr);

impl Index for PointerNode {
    #[inline]
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        match self {
            PointerNode::Index(i) => i.value_index_into(v),
            PointerNode::Key(k) => k.value_index_into(v),
        }
    }

    #[inline]
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        match self {
            PointerNode::Index(i) => i.index_into_mut(v),
            PointerNode::Key(k) => k.index_into_mut(v),
        }
    }

    #[inline]
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        match self {
            PointerNode::Index(i) => i.index_or_insert(v),
            PointerNode::Key(k) => k.index_or_insert(v),
        }
    }

    #[inline]
    fn as_index(&self) -> Option<usize> {
        match self {
            PointerNode::Index(i) => Some(*i),
            PointerNode::Key(_) => None,
        }
    }

    #[inline]
    fn as_key(&self) -> Option<&str> {
        match self {
            PointerNode::Index(_) => None,
            PointerNode::Key(k) => Some(k.as_ref()),
        }
    }
}

impl<T> Index for &T
where
    T: ?Sized + Index,
{
    #[inline]
    fn value_index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        (**self).value_index_into(v)
    }

    #[inline]
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        (**self).index_into_mut(v)
    }

    #[inline]
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        (**self).index_or_insert(v)
    }

    #[inline]
    fn as_index(&self) -> Option<usize> {
        (**self).as_index()
    }

    #[inline]
    fn as_key(&self) -> Option<&str> {
        (**self).as_key()
    }
}
