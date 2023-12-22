use std::{
    fmt::Debug,
    iter::FusedIterator,
    ops::{Deref, DerefMut, Range, RangeBounds},
    ptr::NonNull,
    slice::{from_raw_parts, from_raw_parts_mut},
};

use super::shared::{Shared, SharedCtxGuard};
use crate::{
    serde::tri,
    util::arc::Arc,
    value::{node::Value, value_trait::JsonValueTrait},
};

/// Array represents a JSON array. Its APIs are likes `Array<Value>`.
///
/// # Example
/// ```
/// use sonic_rs::{array, Array, JsonContainerTrait};
///
/// let mut arr: Array = sonic_rs::from_str("[1, 2, 3]").unwrap();
/// assert_eq!(arr[0], 1);
///
/// let mut arr = array![1, 2, 3];
/// assert_eq!(arr[0], 1);
///
/// let j = sonic_rs::json!([1, 2, 3]);
/// assert_eq!(j.as_array().unwrap()[0], 1);
/// ```
#[derive(Debug, Eq, PartialEq, Clone)]
#[repr(transparent)]
pub struct Array(pub(crate) Value);

pub(crate) const DEFAULT_ARRAY_CAP: usize = 8;

impl Array {
    /// Returns the inner [`Value`].
    #[inline]
    pub fn into_value(self) -> Value {
        self.0
    }

    /// Constructs a new, empty `Array`.
    ///
    /// The array will not allocate until elements are pushed onto it.
    #[inline]
    pub const fn new() -> Self {
        let value = Value {
            meta: super::node::Meta::new(super::node::ROOT_ARRAY, std::ptr::null()),
            data: super::node::Data {
                achildren: std::ptr::null_mut(),
            },
        };
        Array(value)
    }

    /// Constructs a new, empty `Array` with at least the specified capacity.
    ///
    /// The array will be able to hold at least `capacity` elements without
    /// reallocating. This method is allowed to allocate for more elements than
    /// `capacity`. If `capacity` is 0, the array will not allocate.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut array = Self::new();
        array.reserve(capacity);
        array
    }

    /// Reserves capacity for at least `additional` more elements to be inserted
    /// in the given `Array`. The collection may reserve more space to
    /// speculatively avoid frequent reallocations. After calling `reserve`,
    /// capacity will be greater than or equal to `self.len() + additional`.
    /// Does nothing if capacity is already sufficient.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3];
    /// arr.reserve(10);
    /// assert!(arr.capacity() >= 13);
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve::<Value>(additional);
    }

    /// Resizes the `Array` in-place so that `len` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len`, the `Array` is extended by the
    /// difference, with each additional slot filled with the `Value` converted from `value`.
    /// If `new_len` is less than `len`, the `Array` is simply truncated.
    ///
    /// If you need more flexibility, use [`Array::resize_with`].
    /// If you only need to resize to a smaller size, use [`Array::truncate`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{array, json};
    ///
    /// let mut arr = array!["hello"];
    /// arr.resize(3, "world");
    /// assert_eq!(arr, ["hello", "world", "world"]);
    ///
    /// arr.resize(2, 0);
    /// assert_eq!(arr, ["hello", "world"]);
    ///
    /// arr.resize(4, json!("repeat"));
    /// assert_eq!(arr, array!["hello", "world", "repeat", "repeat"]);
    /// ```
    #[inline]
    pub fn resize<T: Clone + Into<Value>>(&mut self, new_len: usize, value: T) {
        if new_len > self.len() {
            self.reserve(new_len - self.len());
            for _ in self.len()..new_len {
                self.push(value.clone().into());
            }
        } else {
            self.truncate(new_len);
        }
    }

    /// Resizes the `Array` in-place so that `len` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len`, the `Array` is extended by the
    /// difference, with each additional slot filled with the result of
    /// calling the closure `f`. The return values from `f` will end up
    /// in the `Array` in the order they have been generated.
    ///
    /// If `new_len` is less than `len`, the `Array` is simply truncated.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3];
    /// arr.resize_with(5, Default::default);
    /// assert_eq!(arr, array![1, 2, 3, null, null]);
    ///
    /// let mut arr = array![];
    /// let mut p = 1;
    /// arr.resize_with(4, || {
    ///     p *= 2;
    ///     p.into()
    /// });
    /// assert_eq!(arr, [2, 4, 8, 16]);
    /// ```
    #[inline]
    pub fn resize_with<F>(&mut self, new_len: usize, mut f: F)
    where
        F: FnMut() -> Value,
    {
        if new_len > self.len() {
            self.reserve(new_len - self.len());
            for _ in self.len()..new_len {
                self.push(f());
            }
        } else {
            self.truncate(new_len);
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` for which `f(&e)` returns `false`.
    /// This method operates in place, visiting each element exactly once in the
    /// original order, and preserves the order of the retained elements.
    ///
    /// Because the elements are visited exactly once in the original order,
    /// external state may be used to decide which elements to keep.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3, 4, 5];
    /// let keep = [false, true, true, false, true];
    /// let mut iter = keep.iter();
    /// arr.retain(|_| *iter.next().unwrap());
    /// assert_eq!(arr, array![2, 3, 5]);
    /// ```
    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&Value) -> bool,
    {
        self.retain_mut(|elem| f(elem));
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated array containing the elements in the range
    /// `[at, len)`. After the call, the original array will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3];
    /// let arr2 = arr.split_off(1);
    /// assert_eq!(arr, [1]);
    /// assert_eq!(arr2, [2, 3]);
    /// assert_eq!(arr.split_off(1), array![]);
    /// ```
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        let len = self.len();
        assert!(at <= len, "at {} out of bounds(len: {})", at, len);

        let mut arr = Self::new_in(self.0.shared_clone());
        if at == len {
            return arr;
        }
        arr.reserve(len - at);

        unsafe {
            let src = self.as_mut_ptr().add(at);
            let dst = arr.as_mut_ptr();
            std::ptr::copy_nonoverlapping(src, dst, len - at);
            self.set_len(at);
            arr.set_len(len - at);
        }
        arr
    }

    /// Removes an element from the array and returns it.
    ///
    /// The removed element is replaced by the last element of the array.
    ///
    /// This does not preserve ordering, but is *O*(1).
    /// If you need to preserve the element order, use [`remove`] instead.
    ///
    /// [`remove`]: Array::remove
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut v = array!["foo", "bar", "baz", "qux"];
    ///
    /// assert_eq!(v.swap_remove(1), "bar");
    /// assert_eq!(v, ["foo", "qux", "baz"]);
    ///
    /// assert_eq!(v.swap_remove(0), "foo");
    /// assert_eq!(v, ["baz", "qux"]);
    /// ```
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> Value {
        let len = self.len();
        assert!(index < len, "index {} out of bounds(len: {})", index, len);
        if index != self.len() - 1 {
            unsafe {
                let src = self.as_mut_ptr().add(index);
                let dst = self.as_mut_ptr().add(len - 1);
                std::ptr::swap(src, dst);
            }
        }
        self.pop().unwrap()
    }

    /// Retains only the elements specified by the predicate, passing a mutable reference to it.
    ///
    /// In other words, remove all elements `e` such that `f(&mut e)` returns `false`.
    /// This method operates in place, visiting each element exactly once in the
    /// original order, and preserves the order of the retained elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{array, JsonValueTrait};
    ///
    /// let mut arr = array![1, 2, 3, 4];
    /// arr.retain_mut(|x| {
    ///     let v = (x.as_i64().unwrap());
    ///     if v <= 3 {
    ///         *x = (v + 1).into();
    ///         true
    ///     } else {
    ///         false
    ///     }
    /// });
    /// assert_eq!(arr, [2, 3, 4]);
    /// ```
    #[inline]
    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Value) -> bool,
    {
        if self.is_empty() {
            return;
        }

        let mut i = 0;
        let mut j = 0;
        let start = self.as_mut_ptr();
        while i < self.len() {
            unsafe {
                let cur = start.add(i);
                if !f(&mut *cur) {
                    (*cur).take();
                    i += 1;
                    continue;
                }

                if i > j {
                    std::ptr::copy_nonoverlapping(cur, start.add(j), 1);
                }
                i += 1;
                j += 1;
            }
        }

        unsafe { self.set_len(j) };
    }

    /// Shortens the array, keeping the first `len` elements and dropping
    /// the rest.
    ///
    /// If `len` is greater or equal to the array's current length, this has
    /// no effect.
    ///
    /// The [`drain`] method can emulate `truncate`, but causes the excess
    /// elements to be returned instead of dropped.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the array.
    ///
    /// # Examples
    ///
    /// Truncating a five element array to two elements:
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3, true, "hi"];
    /// arr.truncate(2);
    /// assert_eq!(arr, [1, 2]);
    /// ```
    ///
    /// No truncation occurs when `len` is greater than the array's current
    /// length:
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3];
    /// arr.truncate(8);
    /// assert_eq!(arr, [1, 2, 3]);
    /// ```
    ///
    /// Truncating when `len == 0` is equivalent to calling the [`clear`]
    /// method.
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2, 3];
    /// arr.truncate(0);
    /// assert!(arr.is_empty());
    /// ```
    ///
    /// [`clear`]: Array::clear
    /// [`drain`]: Array::drain
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        let old_len = self.len();
        if len < old_len {
            unsafe {
                let mut v = self.as_mut_ptr().add(len);
                let end = self.as_mut_ptr().add(old_len);

                while v != end {
                    (*v).take();
                    v = v.add(1);
                }
            }
            unsafe { self.set_len(len) };
        }
    }

    /// Appends an element `val` to the back of a collection.
    /// The `val` will be converted into `Value`.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::array;
    /// let mut arr = array![1, 2];
    /// arr.push(3);
    /// arr.push("hi");
    /// assert_eq!(arr, array![1, 2, 3, "hi"]);
    /// ```
    #[inline]
    pub fn push<T: Into<Value>>(&mut self, val: T) {
        self.reserve(1);
        let val = {
            let _ = SharedCtxGuard::assign(self.0.shared());
            val.into()
        };
        self.0.append_value(val);
    }

    /// Removes the last element from a array and returns it, or [`None`] if it is empty.
    #[inline]
    pub fn pop(&mut self) -> Option<Value> {
        debug_assert!(self.0.is_array());
        self.0.pop()
    }

    /// Returns the number of elements in the array.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the array contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    /// Extracts a mutable slice of the entire array. Equivalent to &mut s[..].
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Value] {
        self
    }

    /// Extracts a slice containing the entire array. Equivalent to &s[..].
    #[inline]
    pub fn as_slice(&self) -> &[Value] {
        self
    }

    /// Returns the total number of elements the array can hold without reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Clears the array, removing all values.
    ///
    /// Note that this method has no effect on the allocated capacity of the array.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Removes and returns the element at position `index` within the array,
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::array;
    ///
    /// let mut arr = array![0, 1, 2];
    /// arr.remove(1);
    /// assert_eq!(arr, [0, 2]);
    /// ```
    #[inline]
    pub fn remove(&mut self, index: usize) {
        self.0.remove_index(index);
    }

    /// Moves all the elements of other into self, leaving other empty.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::{array, Value};
    ///
    /// let mut arr1 = array![1];
    /// arr1.push(Value::from("arr1"));
    ///
    /// let mut arr2 = array![2];
    /// arr2.push(Value::from("arr2"));
    /// arr2.append(&mut arr1);
    ///
    /// assert_eq!(arr2, array![2, "arr2", "arr1", 1]);
    /// assert!(arr1.is_empty());
    /// ```
    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.reserve(other.len());
        while let Some(v) = other.pop() {
            debug_assert!(v.is_root() || v.is_static());
            self.push(v);
        }
    }

    /// Removes the specified range from the array in bulk, returning all
    /// removed elements as an iterator. If the iterator is dropped before
    /// being fully consumed, it drops the remaining removed elements.
    ///
    /// The returned iterator keeps a mutable borrow on the array to optimize
    /// its implementation.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the array.
    ///
    /// # Leaking
    ///
    /// If the returned iterator goes out of scope without being dropped (due to
    /// [`std::mem::forget`], for example), the array may have lost and leaked
    /// elements arbitrarily, including elements outside the range.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{array, Value};
    /// let mut v = array![1, 2, 3];
    /// let u: Vec<Value> = v.drain(1..).collect();
    /// assert_eq!(v, &[1]);
    /// assert_eq!(u, &[2, 3]);
    ///
    /// // A full range clears the array, like `clear()` does
    /// v.drain(..);
    /// assert!(v.is_empty());
    /// ```
    #[inline]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_>
    where
        R: RangeBounds<usize>,
    {
        let len = self.len();
        let Range { start, end } = std::slice::range(range, ..len);

        unsafe {
            // set self.arr length's to start, to be safe in case Drain is leaked
            self.set_len(start);
            let range_slice = std::slice::from_raw_parts(self.as_ptr().add(start), end - start);
            Drain {
                tail_start: end,
                tail_len: len - end,
                iter: range_slice.iter(),
                arr: NonNull::from(self),
            }
        }
    }

    /// Copies elements from `src` range to the end of the array.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the array.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::Array;
    /// let mut arr: Array = sonic_rs::from_str("[0, 1, 2, 3, 4]").unwrap();
    ///
    /// arr.extend_from_within(2..);
    /// assert_eq!(arr, [0, 1, 2, 3, 4, 2, 3, 4]);
    ///
    /// arr.extend_from_within(..2);
    /// assert_eq!(arr, [0, 1, 2, 3, 4, 2, 3, 4, 0, 1]);
    ///
    /// arr.extend_from_within(4..8);
    /// assert_eq!(arr, [0, 1, 2, 3, 4, 2, 3, 4, 0, 1, 4, 2, 3, 4]);
    /// ```
    pub fn extend_from_within<R>(&mut self, src: R)
    where
        R: RangeBounds<usize>,
    {
        let range = std::slice::range(src, ..self.len());
        if range.is_empty() {
            return;
        }

        self.reserve(range.len());
        unsafe {
            let start = self.as_mut_ptr().add(range.start);
            let end = self.as_mut_ptr().add(range.end);
            let src = std::slice::from_raw_parts(start, end.offset_from(start) as usize);
            for v in src.iter() {
                self.0.append_value(v.clone_in(self.0.shared()));
            }
        }
    }

    /// Inserts an element at position `index` within the array, shifting all
    /// elements after it to the right.
    /// The `element` will be converted into `Value`.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{array, json};
    ///
    /// let mut arr = array![1, 2, 3];
    /// arr.insert(1, "inserted1");
    /// assert_eq!(arr, array![1, "inserted1", 2, 3]);
    ///
    /// arr.insert(4, "inserted2");
    /// assert_eq!(arr, array![1, "inserted1", 2, 3, "inserted2"]);
    ///
    /// arr.insert(5, json!({"a": 123})); // insert at the end
    /// assert_eq!(arr, array![1, "inserted1", 2, 3, "inserted2", {"a": 123}]);
    /// ```
    #[inline]
    pub fn insert<T: Into<Value>>(&mut self, index: usize, element: T) {
        let element = {
            let _ = SharedCtxGuard::assign(self.0.shared());
            element.into()
        };
        self.0.insert_value(index, element);
    }

    #[inline]
    pub(crate) fn new_in(shared: Arc<Shared>) -> Self {
        let mut array = Array::default();
        array.0.mark_shared(shared.data_ptr());
        std::mem::forget(shared);
        array
    }

    #[inline]
    pub(crate) unsafe fn set_len(&mut self, new_len: usize) {
        self.0.set_len(new_len);
    }
}

impl Default for Array {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Array {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let start = self.0.data.achildren.add(Value::MEAT_NODE_COUNT);
            let ptr = start as *const Value;
            let len = self.0.len();
            from_raw_parts(ptr, len)
        }
    }
}

impl DerefMut for Array {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let ptr = self.0.data.achildren.add(Value::MEAT_NODE_COUNT);
            let len = self.0.len();
            from_raw_parts_mut(ptr, len)
        }
    }
}

/// A draining iterator for `Array<T>`.
///
/// This `struct` is created by [`Array::drain`].
/// See its documentation for more.
pub struct Drain<'a> {
    pub(super) tail_start: usize,
    pub(super) tail_len: usize,
    // the iter of remain slice
    pub(super) iter: std::slice::Iter<'a, Value>,
    // origin array
    pub(super) arr: NonNull<Array>,
}

impl<'a> Drain<'a> {
    /// Returns the remaining items of its iterator as a slice.
    #[inline]
    pub fn as_slice(&self) -> &'a [Value] {
        self.iter.as_slice()
    }
}

impl Iterator for Drain<'_> {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|v| v.clone_in(unsafe { self.arr.as_ref().0.shared() }))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

use std::{
    ops::{Index, IndexMut},
    slice::SliceIndex,
};

impl<I: SliceIndex<[Value]>> Index<I> for Array {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&**self, index)
    }
}

impl<I: SliceIndex<[Value]>> IndexMut<I> for Array {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut **self, index)
    }
}

//////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Default, Clone)]
pub struct IntoIter {
    array: Array,
    index: usize,
    len: usize,
}

impl IntoIter {
    pub fn as_mut_slice(&mut self) -> &mut [Value] {
        unsafe {
            let ptr = self.array.0.children_mut_ptr();
            let len = self.array.0.len();
            from_raw_parts_mut(ptr, len)
        }
    }

    pub fn as_slice(&self) -> &[Value] {
        unsafe {
            let ptr = self.array.0.children_ptr();
            let len = self.array.0.len();
            from_raw_parts(ptr, len)
        }
    }
}

impl AsRef<[Value]> for IntoIter {
    fn as_ref(&self) -> &[Value] {
        self.as_slice()
    }
}

impl AsMut<[Value]> for IntoIter {
    fn as_mut(&mut self) -> &mut [Value] {
        self.as_mut_slice()
    }
}

impl DoubleEndedIterator for IntoIter {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            self.len -= 1;
            let value = self.array.0.get_index_mut(self.len).unwrap();
            Some(value.take())
        } else {
            None
        }
    }
}

impl ExactSizeIterator for IntoIter {
    #[inline]
    fn len(&self) -> usize {
        self.len - self.index
    }
}

impl FusedIterator for IntoIter {}

impl Iterator for IntoIter {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            let value = self.array.0.get_index_mut(self.index).unwrap();
            self.index += 1;
            Some(value.take())
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len - self.index;
        (len, Some(len))
    }
}

impl IntoIterator for Array {
    type Item = Value;
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        IntoIter {
            array: self,
            index: 0,
            len,
        }
    }
}

impl<'a> IntoIterator for &'a Array {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Array {
    type Item = &'a mut Value;
    type IntoIter = std::slice::IterMut<'a, Value>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

//////////////////////////////////////////////////////////////////////////////

impl serde::ser::Serialize for Array {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = tri!(serializer.serialize_seq(Some(self.len())));
        for v in self {
            tri!(seq.serialize_element(v));
        }
        seq.end()
    }
}

impl<'de> serde::de::Deserialize<'de> for Array {
    #[inline]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        // deserialize to value at first
        let value: Value =
            deserializer.deserialize_newtype_struct(super::de::TOKEN, super::de::ValueVisitor)?;
        if value.is_array() {
            Ok(Array(value))
        } else {
            Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Other("not a array"),
                &"array",
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use super::Array;
    use crate::value::{node::Value, value_trait::JsonValueMutTrait};

    #[test]
    fn test_value_array() {
        let mut val = crate::from_str::<Value>(r#"[1,2,3]"#);
        let array = val.as_array_mut().unwrap();
        assert_eq!(array.len(), 3);

        for i in 0..3 {
            // push static node
            let old_len = array.len();
            let new_node = Value::new_u64(i, std::ptr::null());
            array.push(new_node);
            assert_eq!(array.len(), old_len + 1);

            // push node with new allocator
            let old_len = array.len();
            let mut new_node = Array::default();
            new_node.push(Value::new_u64(i, std::ptr::null()));
            dbg!(&new_node.0);
            array.push(new_node.0);
            assert_eq!(array.len(), old_len + 1);

            // push node with self allocator
            let old_len = array.len();
            let mut new_node = Array::new_in(array.0.shared_clone());
            new_node.push(Value::new_u64(i, std::ptr::null()));
            dbg!(&new_node.0);
            array.push(new_node.0);
            assert_eq!(array.len(), old_len + 1);
        }

        dbg!(&array);
        for (i, v) in array.iter_mut().enumerate() {
            *v = Value::new_u64(i as u64, std::ptr::null());
        }

        while !array.is_empty() {
            dbg!(array.pop());
        }
    }
}
