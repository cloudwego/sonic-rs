//! Represents a parsed JSON object.
use std::marker::PhantomData;

use super::{
    node::replace_value,
    shared::{Shared, SharedCtxGuard},
    value_trait::JsonValueTrait,
};
use crate::{serde::tri, util::reborrow::DormantMutRef, value::node::Value};

/// Represents the JSON object. The inner implement is a key-value array. Its order is as same as
/// origin JSON.
///
/// # Examples
/// ```
/// use sonic_rs::{from_str, Object};
///
/// let mut obj: Object = from_str(r#"{"a": 1, "b": true, "c": null}"#).unwrap();
///
/// assert_eq!(obj["a"], 1);
/// assert_eq!(obj.insert(&"d", "e"), None);
/// assert_eq!(obj["d"], "e");
/// assert_eq!(obj.len(), 4);
/// ```
///
/// # Warning
/// The key in `Object` is not sorted and the `get` operation is O(n). And `Object` is allowed to
/// have duplicated keys.
///
/// # Examples
/// ```
/// use sonic_rs::{from_str, Object};
///
/// let obj: Object = from_str(r#"{"a": 1, "a": true, "a": null}"#).unwrap();
///
/// assert_eq!(obj["a"], 1);
/// assert_eq!(obj.len(), 3); // allow duplicated keys
/// ```
/// If you care about that, recommend to use `HashMap` or `BTreeMap` instead. The parse performance
/// is slower than `Object`.
#[derive(Debug, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Object(pub(crate) Value);

impl Default for Object {
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
pub type Pair = (Value, Value);

pub(crate) const DEFAULT_OBJ_CAP: usize = 4;

impl Object {
    /// Returns the inner `Value`.
    #[inline]
    pub fn into_value(self) -> Value {
        self.0
    }

    /// Create a new empty object.
    ///
    /// # Example
    /// ```
    /// use sonic_rs::{from_str, json, object, prelude::*, Object};
    ///
    /// let mut obj: Object = from_str("{}").unwrap();
    /// obj.insert(&"arr", object! {});
    /// obj.insert(&"a", 1);
    /// obj.insert(&"arr2", Object::new());
    /// obj["a"] = json!(123);
    /// obj["arr2"] = json!("hello world");
    ///
    /// assert_eq!(obj["a"], 123);
    /// assert_eq!(obj["arr2"], "hello world");
    /// ```
    #[inline]
    pub const fn new() -> Object {
        let value = Value {
            meta: super::node::Meta::new(super::node::OBJECT, std::ptr::null()),
            data: super::node::Data {
                achildren: std::ptr::null_mut(),
            },
        };
        Object(value)
    }

    /// Create a new empty object with capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut v = Self::new();
        v.0.reserve::<Pair>(capacity);
        v
    }

    /// Clear the object, make it as empty but keep the allocated memory.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, object};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.clear();
    /// assert!(obj.is_empty());
    /// assert!(obj.capacity() >= 3);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Returns the capacity of the object.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, object};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.insert(&"d", "e");
    /// assert_eq!(obj.get(&"d").unwrap(), "e");
    /// assert_eq!(obj.get(&"f"), None);
    /// assert_eq!(obj.get(&"a").unwrap(), 1);
    /// ```
    #[inline]
    pub fn get<Q: AsRef<str>>(&self, key: &Q) -> Option<&Value> {
        self.0.get_key(key.as_ref())
    }

    /// Returns `true` if the map contains a value for the specified key.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::object;
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.insert(&"d", "e");
    /// assert_eq!(obj.contains_key(&"d"), true);
    /// assert_eq!(obj.contains_key(&"a"), true);
    /// assert_eq!(obj.contains_key(&"e"), false);
    /// ```
    #[inline]
    pub fn contains_key<Q: AsRef<str>>(&self, key: &Q) -> bool {
        self.get(key).is_some()
    }

    /// Returns a mutable reference to the value corresponding to the key.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::object;
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.insert(&"d", "e");
    ///
    /// *(obj.get_mut(&"d").unwrap()) = "f".into();
    /// assert_eq!(obj.contains_key(&"d"), true);
    /// assert_eq!(obj["d"], "f");
    /// ```
    #[inline]
    pub fn get_mut<Q: AsRef<str>>(&mut self, key: &Q) -> Option<&mut Value> {
        self.0.get_key_mut(key.as_ref()).map(|v| v.0)
    }

    /// Returns the key-value pair corresponding to the supplied key.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, Value};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.insert(&"d", "e");
    ///
    /// assert_eq!(obj.get_key_value(&"d").unwrap(), ("d", &Value::from("e")));
    /// assert_eq!(obj.get_key_value(&"a").unwrap(), ("a", &Value::from(1)));
    /// assert_eq!(obj.get_key_value(&"e"), None);
    /// ```
    #[inline]
    pub fn get_key_value<Q: AsRef<str>>(&self, key: &Q) -> Option<(&str, &Value)> {
        self.0.get_key_value(key.as_ref())
    }

    /// Inserts a key-value pair into the object. The `Value` is converted from `V`.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// If the object did not have this key present, [`None`] is returned.
    ///
    /// If the object did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical. See the [module-level
    /// documentation] for more.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, object, Value};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// assert_eq!(obj.len(), 3);
    /// assert_eq!(obj.insert(&"d", "e"), None);
    /// assert_eq!(obj.len(), 4);
    /// assert_eq!(obj["d"], "e");
    /// assert_eq!(obj.insert(&"d", "f").unwrap(), "e");
    /// assert_eq!(obj["d"], "f");
    /// assert_eq!(obj.len(), 4);
    /// assert_eq!(obj.insert(&"d", json!("h")).unwrap(), "f");
    /// assert_eq!(obj["d"], "h");
    /// assert_eq!(obj.insert(&"i", Value::from("j")), None);
    /// assert_eq!(obj.len(), 5);
    /// ```
    #[inline]
    pub fn insert<K: AsRef<str>, V: Into<Value>>(&mut self, key: &K, value: V) -> Option<Value> {
        match self.entry(key) {
            Entry::Occupied(mut entry) => Some(entry.insert(value)),
            Entry::Vacant(entry) => {
                entry.insert(value);
                None
            }
        }
    }

    /// Removes a key from the object, returning the value at the key if the key
    /// was previously in the object.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::object;
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// assert_eq!(obj.remove(&"d"), None);
    /// assert_eq!(obj.remove(&"a").unwrap(), 1);
    /// ```
    #[inline]
    pub fn remove<Q: AsRef<str>>(&mut self, key: &Q) -> Option<Value> {
        self.0.remove_key(key.as_ref())
    }

    /// Removes a key from the object, returning the stored key and value if the
    /// key was previously in the obj.
    ///
    /// The key may be [`AsRef<str>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::object;
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// assert_eq!(obj.remove_entry(&"d"), None);
    /// let (key, val) = obj.remove_entry(&"a").unwrap();
    /// assert_eq!(key, "a");
    /// assert_eq!(val, 1);
    /// ```
    #[inline]
    pub fn remove_entry<'k, Q: AsRef<str>>(&mut self, key: &'k Q) -> Option<(&'k str, Value)> {
        self.0.remove_key(key.as_ref()).map(|v| (key.as_ref(), v))
    }

    /// Returns the number of key-value paris in the object.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the object contains no key-value pairs.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an immutable iterator over the key-value pairs of the object.
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.iter::<Pair>())
    }

    /// Returns an mutable iterator over  the key-value pairs of the object.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut(self.0.iter_mut::<Pair>())
    }

    /// Gets the given key's corresponding entry in the object for in-place manipulation.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, Value};
    ///
    /// let mut obj = object! {};
    ///
    /// for i in 0..10 {
    ///     obj.entry(&i.to_string()).or_insert(1);
    /// }
    ///
    /// for i in 0..10 {
    ///     obj.entry(&i.to_string())
    ///         .and_modify(|v| *v = Value::from(i + 1));
    /// }
    ///
    /// assert_eq!(obj[&"1"], 2);
    /// assert_eq!(obj[&"2"], 3);
    /// assert_eq!(obj[&"3"], 4);
    /// assert_eq!(obj.get(&"10"), None);
    /// ```
    #[inline]
    pub fn entry<'a, Q: AsRef<str>>(&'a mut self, key: &Q) -> Entry<'a> {
        let (obj, mut dormant_obj) = DormantMutRef::new(self);
        match obj.0.get_key_mut(key.as_ref()) {
            None => {
                // check flat
                let obj_re = unsafe { dormant_obj.reborrow() };
                if obj_re.0.is_static() {
                    obj_re.0.mark_shared(Shared::new_ptr());
                    obj_re.0.mark_root();
                }
                let shared = obj_re.0.shared();
                let key = Value::copy_str(key.as_ref(), shared);
                Entry::Vacant(VacantEntry {
                    key,
                    dormant_obj,
                    _marker: PhantomData,
                })
            }
            Some((handle, offset)) => {
                Entry::Occupied(OccupiedEntry::new(handle, offset, dormant_obj))
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all pairs `(k, v)` for which `f(&k, &mut v)` returns `false`.
    /// The elements are visited in unsorted (and unspecified) order.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::object;
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.retain(|key, _| key == "a");
    /// assert_eq!(obj.len(), 1);
    /// assert_eq!(obj["a"], 1);
    /// ```
    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&str, &mut Value) -> bool,
    {
        if self.is_empty() {
            return;
        }
        let start: *mut Pair = unsafe { self.0.children_mut_ptr::<Pair>() };
        let mut cur = start;
        for (k, v) in self.0.iter_mut::<Pair>() {
            let key = k.as_str().unwrap();
            if f(key, v) {
                if !std::ptr::eq(k, cur as *mut Value) {
                    unsafe { std::ptr::copy_nonoverlapping((k as *mut Value).cast(), cur, 1) };
                }
                cur = unsafe { cur.add(1) };
            } else {
                // drop the old value
                v.take();
            }
        }
        unsafe {
            let new_len = cur.offset_from(start) as usize;
            self.0.set_len(new_len);
        }
    }

    /// Moves all elements from other into self, leaving other empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, object};
    ///
    /// let mut a = object! {};
    /// let mut b = object! {"a": null, "b": 1};
    /// a.append(&mut b);
    ///
    /// assert_eq!(a, object! {"a": null, "b": 1});
    /// assert!(b.is_empty());
    /// ```
    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        while let Some((k, v)) = other.0.pop_pair() {
            self.0.append_pair((k, v));
        }
    }

    /// Reserves capacity for at least additional more elements to be inserted in the given.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::object;
    /// let mut obj = object! {};
    /// obj.reserve(1);
    /// assert!(obj.capacity() >= 1);
    ///
    /// obj.reserve(10);
    /// assert!(obj.capacity() >= 10);
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve::<Pair>(additional);
    }
}

/// A view into a single occupied location in a `Object`.
pub struct OccupiedEntry<'a> {
    handle: &'a mut Value,
    offset: usize,
    dormant_obj: DormantMutRef<'a, Object>,
    _marker: PhantomData<&'a mut Pair>,
}

impl<'a> OccupiedEntry<'a> {
    /// Gets a reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, value::object::Entry, Value};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    ///
    /// if let Entry::Occupied(entry) = obj.entry(&"a") {
    ///     assert_eq!(entry.get(), 1);
    /// }
    ///
    /// if let Entry::Occupied(entry) = obj.entry(&"b") {
    ///     assert_eq!(entry.get(), true);
    /// }
    /// ```
    #[inline]
    pub fn get(&self) -> &Value {
        self.handle
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, value::object::Entry, Value};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.insert(&"a", Value::from("hello"));
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"a") {
    ///     assert_eq!(entry.get_mut(), &Value::from("hello"));
    /// }
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"b") {
    ///     assert_eq!(entry.get_mut(), &true);
    /// }
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut Value {
        self.handle
    }

    /// Converts the entry into a mutable reference to its value.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, value::object::Entry, Value};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    /// obj.insert(&"a", Value::from("hello"));
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"a") {
    ///     let vref = entry.into_mut();
    ///     assert_eq!(vref, &mut Value::from("hello"));
    ///     *vref = Value::from("world");
    /// }
    ///
    /// assert_eq!(obj["a"], "world");
    /// ```
    #[inline]
    pub fn into_mut(self) -> &'a mut Value {
        self.handle
    }

    /// Sets the value of the entry, and returns the entry's old value.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, value::object::Entry};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"a") {
    ///     assert_eq!(entry.insert("hello"), 1);
    /// }
    /// if let Entry::Occupied(mut entry) = obj.entry(&"a") {
    ///     assert_eq!(entry.insert("world"), "hello");
    /// }
    /// ```
    #[inline]
    pub fn insert<T: Into<Value>>(&mut self, val: T) -> Value {
        let obj = unsafe { self.dormant_obj.reborrow() };
        let val = {
            let _ = SharedCtxGuard::assign(obj.0.shared_parts());
            val.into()
        };
        replace_value(self.handle, val)
    }

    /// Takes the value out of the entry, and returns it.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{object, value::object::Entry, Value};
    ///
    /// let mut obj = object! {"a": 1, "b": true, "c": null};
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"a") {
    ///     assert_eq!(entry.remove(), 1);
    /// }
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"b") {
    ///     assert_eq!(entry.remove(), true);
    /// }
    ///
    /// if let Entry::Occupied(mut entry) = obj.entry(&"c") {
    ///     assert_eq!(entry.remove(), Value::default());
    /// }
    /// assert!(obj.is_empty());
    /// ```
    #[inline]
    pub fn remove(mut self) -> Value {
        let obj = unsafe { self.dormant_obj.reborrow() };
        let (_, val) = obj.0.remove_pair_index(self.offset);
        val
    }

    #[inline]
    pub(crate) fn new(
        handle: &'a mut Value,
        offset: usize,
        dormant_obj: DormantMutRef<'a, Object>,
    ) -> Self {
        Self {
            handle,
            offset,
            dormant_obj,
            _marker: PhantomData,
        }
    }
}

/// A view into a vacant entry in a `Object`.
pub struct VacantEntry<'a> {
    pub(super) key: Value,
    pub(super) dormant_obj: DormantMutRef<'a, Object>,
    pub(super) _marker: PhantomData<&'a mut Pair>,
}

impl<'a> VacantEntry<'a> {
    /// Insert a value into the vacant entry and return a mutable reference to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{json, object, value::object::Entry};
    ///
    /// let mut obj = object! {};
    ///
    /// if let Entry::Vacant(entry) = obj.entry(&"hello") {
    ///     assert_eq!(entry.insert(1), &1);
    /// }
    ///
    /// if let Entry::Vacant(entry) = obj.entry(&"world") {
    ///     assert_eq!(entry.insert(json!("woo").clone()), "woo");
    /// }
    ///
    /// assert_eq!(obj.get(&"hello").unwrap(), 1);
    /// assert_eq!(obj.get(&"world").unwrap(), "woo");
    /// ```
    pub fn insert<T: Into<Value>>(self, val: T) -> &'a mut Value {
        let obj = unsafe { self.dormant_obj.awaken() };
        obj.reserve(1);
        let val = {
            let _ = SharedCtxGuard::assign(obj.0.shared_parts());
            val.into()
        };
        let pair = obj.0.append_pair((self.key, val));
        &mut pair.1
    }

    /// Get the key of the vacant entry.
    pub fn key(&self) -> &str {
        self.key.as_str().unwrap()
    }
}

/// A view into a single entry in a map, which may either be vacant or occupied.
pub enum Entry<'a> {
    /// A vacant Entry.
    Vacant(VacantEntry<'a>),
    /// An occupied Entry.
    Occupied(OccupiedEntry<'a>),
}

impl<'a> Entry<'a> {
    /// Ensures a value is in the entry by inserting the default if empty,
    /// Example:
    /// ```rust
    /// use sonic_rs::object;
    ///
    /// let mut obj = object! {};
    /// obj.entry(&"hello").or_insert(1);
    /// assert_eq!(obj.get(&"hello").unwrap(), 1);
    ///
    /// obj.entry(&"hello").or_insert(2);
    /// assert_eq!(obj.get(&"hello").unwrap(), 1);
    /// ```
    #[inline]
    pub fn or_insert<T: Into<Value>>(self, default: T) -> &'a mut Value {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// Example:
    /// ```rust
    /// use sonic_rs::Object;
    /// let mut obj = Object::new();
    /// obj.entry(&"hello").or_insert_with(|| 1.into());
    /// assert_eq!(obj.get(&"hello").unwrap(), 1);
    ///
    /// obj.entry(&"hello").or_insert_with(|| 2.into());
    /// assert_eq!(obj.get(&"hello").unwrap(), 1);
    /// ```
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> Value>(self, default: F) -> &'a mut Value {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Return the key of the entry.
    #[inline]
    pub fn key(&self) -> &str {
        match self {
            Entry::Occupied(entry) => entry.handle.as_str().unwrap(),
            Entry::Vacant(entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any potential inserts into the
    /// object.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, value::object::Entry};
    ///
    /// let mut obj = object! {"a": 0, "b": true, "c": null};
    /// obj.entry(&"a").and_modify(|v| *v = 2.into());
    /// assert_eq!(obj.get(&"a").unwrap(), 2);
    ///
    /// obj.entry(&"a")
    ///     .and_modify(|v| *v = 2.into())
    ///     .and_modify(|v| *v = 3.into());
    /// assert_eq!(obj.get(&"a").unwrap(), 3);
    ///
    /// obj.entry(&"d").and_modify(|v| *v = 3.into());
    /// assert_eq!(obj.get(&"d"), None);
    ///
    /// obj.entry(&"d").and_modify(|v| *v = 3.into()).or_insert(4);
    /// assert_eq!(obj.get(&"d").unwrap(), 4);
    /// ```
    #[inline]
    pub fn and_modify<F: FnOnce(&mut Value)>(self, f: F) -> Self {
        match self {
            Entry::Occupied(entry) => {
                f(entry.handle);
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }

    /// Ensures a value is in the entry by inserting the default value if empty, and returns a
    /// mutable reference to the value in the entry. # Examples
    ///
    /// ```
    /// use sonic_rs::{object, value::object::Entry, Value};
    ///
    /// let mut obj = object! {"c": null};
    /// assert_eq!(obj.entry(&"a").or_default(), &Value::default());
    /// assert_eq!(obj.entry(&"d").or_default(), &Value::default());
    /// ```
    #[inline]
    pub fn or_default(self) -> &'a mut Value {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Value::default()),
        }
    }

    /// Ensures a value is in the entry by inserting, if empty, the result of the default function.
    /// This method allows for generating key-derived values for insertion by providing the default
    /// function a reference to the key that was moved during the `.entry(key)` method call.
    ///
    /// The reference to the moved key is provided so that cloning or copying the key is
    /// unnecessary, unlike with `.or_insert_with(|| ... )`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{object, Value};
    ///
    /// let mut obj = object! {"c": null};
    ///
    /// obj.entry(&"a")
    ///     .or_insert_with_key(|key| Value::from(key.len()));
    /// assert_eq!(obj.get(&"a").unwrap(), 1);
    ///
    /// obj.entry(&"b").or_insert_with_key(|key| Value::from(key));
    /// assert_eq!(obj.get(&"b").unwrap(), "b");
    /// ```
    #[inline]
    pub fn or_insert_with_key<F>(self, default: F) -> &'a mut Value
    where
        F: FnOnce(&str) -> Value,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(mut entry) => {
                let obj = unsafe { entry.dormant_obj.reborrow() };
                let value = {
                    let _ = SharedCtxGuard::assign(obj.0.shared_parts());
                    default(entry.key())
                };
                entry.insert(value)
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

use std::{iter::FusedIterator, slice};

macro_rules! impl_entry_iter {
    (($name:ident $($generics:tt)*): $item:ty) => {
        impl $($generics)* Iterator for $name $($generics)* {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|(k, v)| (k.as_str().unwrap(), v))
            }
        }

        impl $($generics)* DoubleEndedIterator for $name $($generics)* {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|(k, v)| (k.as_str().unwrap(), v))
            }
        }

        impl $($generics)* ExactSizeIterator for $name $($generics)* {
            #[inline]
            fn len(&self) -> usize {
                self.0.len()
            }
        }

        impl $($generics)* FusedIterator for $name $($generics)* {}
    };
}

/// An iterator over the entries of a `Object`.
pub struct Iter<'a>(slice::Iter<'a, (Value, Value)>);
impl_entry_iter!((Iter<'a>): (&'a str, &'a Value));

/// A mutable iterator over the entries of a `Object`.
pub struct IterMut<'a>(slice::IterMut<'a, (Value, Value)>);
impl_entry_iter!((IterMut<'a>): (&'a str, &'a mut Value));

/// An iterator over the keys of a `Object`.
pub struct Keys<'a>(Iter<'a>);

impl<'a> Iterator for Keys<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }
}

impl<'a> DoubleEndedIterator for Keys<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, _)| k)
    }
}

impl<'a> ExactSizeIterator for Keys<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> FusedIterator for Keys<'a> {}

macro_rules! impl_value_iter {
    (($name:ident $($generics:tt)*): $item:ty) => {
        impl $($generics)* Iterator for $name $($generics)* {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|(_, v)| v)
            }
        }

        impl $($generics)* DoubleEndedIterator for $name $($generics)* {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|(_, v)| v)
            }
        }

        impl $($generics)* ExactSizeIterator for $name $($generics)* {
            #[inline]
            fn len(&self) -> usize {
                self.0.len()
            }
        }

        impl $($generics)* FusedIterator for $name $($generics)* {}
    };
}

/// An iterator over the values of a `Object`.
pub struct Values<'a>(Iter<'a>);
impl_value_iter!((Values<'a>): &'a Value);

/// A mutable iterator over the values of a `Object`.
pub struct ValuesMut<'a>(IterMut<'a>);
impl_value_iter!((ValuesMut<'a>): &'a mut Value);

impl<'a> IntoIterator for &'a Object {
    type Item = (&'a str, &'a Value);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Object {
    type Item = (&'a str, &'a mut Value);
    type IntoIter = IterMut<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a, Q: AsRef<str> + ?Sized> std::ops::Index<&'a Q> for Object {
    type Output = Value;

    #[inline]
    fn index(&self, index: &'a Q) -> &Self::Output {
        self.get(&index.as_ref()).unwrap()
    }
}

impl<'a, Q: AsRef<str> + ?Sized> std::ops::IndexMut<&'a Q> for Object {
    #[inline]
    fn index_mut(&mut self, index: &'a Q) -> &mut Self::Output {
        self.get_mut(&index.as_ref()).unwrap()
    }
}

//////////////////////////////////////////////////////////////////////////////

impl serde::ser::Serialize for Object {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = tri!(serializer.serialize_map(Some(self.len())));
        for (k, v) in self {
            tri!(map.serialize_entry(k, v));
        }
        map.end()
    }
}

impl<'de> serde::de::Deserialize<'de> for Object {
    #[inline]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        // deserialize to value at first
        let value: Value =
            deserializer.deserialize_newtype_struct(super::de::TOKEN, super::de::ValueVisitor)?;
        if value.is_object() {
            Ok(Object(value))
        } else {
            Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Other("not a object"),
                &"object",
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{from_str, to_string, Array, JsonValueMutTrait};

    #[test]
    fn test_object_serde() {
        let json = r#"{"a": 1, "b": true, "c": null}"#;
        let obj: Object = from_str(json).unwrap();
        assert_eq!(obj, object! {"a": 1, "b": true, "c": null});
        let json = to_string(&obj).unwrap();
        assert_eq!(json, r#"{"a":1,"b":true,"c":null}"#);
    }

    #[test]
    fn test_value_object() {
        let mut val = crate::from_str::<Value>(r#"{"a": 123, "b": "hello"}"#);
        let obj = val.as_object_mut().unwrap();

        for i in 0..3 {
            // push static node
            let new_node = Value::new_u64(i, std::ptr::null());
            obj.insert(&"c", new_node);
            assert_eq!(obj["c"], i);

            // push node with new allocator
            let mut new_node = Array::default();
            new_node.push(Value::new_u64(i, std::ptr::null()));
            obj.insert(&"d", new_node);
            assert_eq!(obj["d"][0], i);

            // push node with self allocator
            let mut new_node = Array::new_in(obj.0.shared_clone());
            new_node.push(Value::new_u64(i, std::ptr::null()));
            obj.insert(&"e", new_node);
            assert_eq!(obj["e"][0], i);
        }

        for (i, v) in obj.iter_mut().enumerate() {
            *(v.1) = Value::from(&i.to_string());
        }

        for (i, v) in obj.iter().enumerate() {
            assert_eq!(v.1, &Value::from(&i.to_string()));
        }
    }
}
