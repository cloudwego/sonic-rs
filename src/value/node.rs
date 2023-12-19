use super::alloctor::SyncBump;
use super::object::Pair;
use super::shared::get_shared_or_new;
use super::shared::Shared;
use super::value_trait::JsonContainerTrait;
use super::value_trait::JsonValueMutTrait;
use super::visitor::JsonVisitor;
use crate::error::Result;
use crate::index::Index;
use crate::parser::Parser;
use crate::pointer::PointerNode;
use crate::reader::PaddedSliceRead;
use crate::reader::Reader;
use crate::serde::tri;
use crate::util::arc::Arc;
use crate::util::taggedptr::TaggedPtr;
use crate::value::alloctor::AllocatorTrait;
use crate::value::array::Array;
use crate::value::object::Object;
use crate::value::value_trait::JsonValueTrait;
use crate::JsonType;
use crate::Number;
use bumpalo::Bump;
use core::mem::size_of;
use serde::ser::{Error, Serialize, SerializeMap, SerializeSeq};
use std::alloc::Layout;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::mem::{transmute, ManuallyDrop, MaybeUninit};
use std::ptr::NonNull;
use std::slice::{from_raw_parts, from_raw_parts_mut};
use std::str::from_utf8_unchecked;

/// Represents any valid JSON value.
/// `Value` can be parsed from a JSON and from any type that implements `serde::Serialize`.
///
/// # Example
/// ```
/// use sonic_rs::json;
/// use sonic_rs::value::Value;
///
/// let v1 = json!({"a": 123});
/// let v2: Value = sonic_rs::from_str(r#"{"a": 123}"#).unwrap();
/// let v3 = {
///     use std::collections::HashMap;
///     let mut map: HashMap<&str, i32> = HashMap::new();
///     map.insert(&"a", 123);
///     sonic_rs::to_value(&map).unwrap()
/// };
///
/// assert_eq!(v1, v2);
/// assert_eq!(v2, v3);
///
/// assert_eq!(v1["a"], 123);
/// ```
///
pub struct Value {
    pub(crate) meta: Meta,
    pub(crate) data: Data,
}

unsafe impl Sync for Value {}
unsafe impl Send for Value {}

impl Clone for Value {
    /// Clone the value, if the value is a root node, we will create a new allocator for it.
    ///
    /// # Example
    ///
    /// ```
    /// use sonic_rs::json;
    ///
    /// let a = json!({"a": [1, 2, 3]});
    /// assert_eq!(a, a.clone());
    ///
    /// ```
    fn clone(&self) -> Self {
        match self.get_type() {
            JsonType::Array | JsonType::Object if !self.is_empty() => {
                let (shared, _) = get_shared_or_new();
                let mut v = self.clone_in(shared);
                v.mark_root();
                v
            }
            JsonType::String => {
                let s = self.str();
                // TODO: optimize static string
                if s.is_empty() {
                    return Value::new_str("", std::ptr::null());
                }
                let (shared, _) = get_shared_or_new();
                let mut v = Value::copy_str(s, shared);
                v.mark_root();
                v
            }
            JsonType::Array if self.is_empty() => Value::new_array(std::ptr::null(), 0),
            JsonType::Object if self.is_empty() => Value::new_object(std::ptr::null(), 0),
            _ => {
                let mut v = Value {
                    meta: self.meta,
                    data: self.data,
                };
                v.mark_shared(std::ptr::null());
                v
            }
        }
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let array_children = if self.is_array() {
            Some(self.children::<Value>().unwrap_or(&[]))
        } else {
            None
        };

        let object_children = if self.is_object() {
            Some(self.children::<Pair>().unwrap_or(&[]))
        } else {
            None
        };

        let shared = if self.is_static() {
            None
        } else {
            Some(self.arc_shared())
        };

        let ret = f
            .debug_struct("Value")
            .field("data", &format!("{}", self))
            .field("is_root", &self.is_root())
            .field("shared_address", &self.meta.ptr())
            .field("shared", &shared)
            .field("array_children", &array_children)
            .field("object_children", &object_children)
            .finish();
        std::mem::forget(shared);
        ret
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::new()
    }
}

impl Value {
    /// Convert into `Object`. If the value is not an object, return `None`.
    ///
    #[inline]
    pub fn into_object(self) -> Option<Object> {
        if self.is_object() {
            Some(Object(self))
        } else {
            None
        }
    }

    /// Convert into `Array`. If the value is not an array, return `None`.
    ///
    #[inline]
    pub fn into_array(self) -> Option<Array> {
        if self.is_array() {
            Some(Array(self))
        } else {
            None
        }
    }

    pub(crate) fn is_root(&self) -> bool {
        (self.meta.tag() & ROOT_MASK) == 0b1100
    }

    pub(crate) fn is_inlined(&self) -> bool {
        self.meta.tag() < STRING && !self.is_static()
    }

    pub(crate) fn is_shared(&self) -> bool {
        !self.is_root() && !self.is_static()
    }

    pub(crate) fn unmark_root(&mut self) {
        let tag = self.meta.tag();
        if tag >= STRING {
            self.meta.set_tag(tag & UNROOT_MASK);
        }
    }

    pub(crate) fn unset_root(&mut self) {
        drop(self.arc_shared());
    }

    #[doc(hidden)]
    #[inline]
    pub fn mark_root(&mut self) {
        let tag = self.meta.tag();
        if tag >= STRING {
            self.meta.set_tag(tag | ROOT_MASK);
        } else {
            self.meta.set_ptr(std::ptr::null());
        }
    }

    pub(crate) fn clone_in(&self, shared: &Shared) -> Self {
        // let arc_shared =
        match self.get_type() {
            JsonType::Array => {
                let mut arr = Value::new_array(shared, self.len());
                for v in self.children::<Value>().unwrap() {
                    arr.append_value(v.clone_in(shared));
                }
                arr
            }
            JsonType::Object => {
                let mut obj = Value::new_object(shared, self.len());
                for (k, v) in self.children::<(Value, Value)>().unwrap() {
                    obj.append_pair((k.clone_in(shared), v.clone_in(shared)));
                }
                obj
            }
            JsonType::String => Value::copy_str(self.as_str().unwrap(), shared),
            _ => {
                let mut v = Value {
                    meta: self.meta,
                    data: self.data,
                };
                v.mark_shared(shared);
                v
            }
        }
    }

    #[inline]
    pub(crate) fn set_type(&mut self, typ: u64) {
        self.meta.set_tag(typ);
    }

    pub(crate) fn drop_slow(&mut self) {
        if self.is_array() {
            for child in self.children_mut_unchecked::<Value>() {
                child.drop_slow();
            }
        } else if self.is_object() {
            for child in self.children_mut_unchecked::<(Value, Value)>() {
                child.0.drop_slow();
                child.1.drop_slow();
            }
        }

        if self.is_root() {
            drop(self.arc_shared());
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", crate::to_string(self).expect("invalid value"))
    }
}

// Value Status:
// IsRoot: have a refcnt for the shared allocator and it is often string, non-empty array, non-empty object
// IsnotRoot: the children nodes, it is owned by the root node
// IsCombined: the children maybe a root, it have other allocators
// IsFlatten: the node or its children donot have any allocators

// The drop policy:
// IsRoot + IsCombined: -> drop traverse
// IsnotRoot + IsCombined: -> drop traverse
// IsRoot + IsnotCombined: -> drop directly, refcnt - 1
// IsnotRoot + IsnotCombined: -> ignore it

// To make sure correctness, when we drop a node that is not a root node, we must mark Shared as combined.
// such as an assignment operation: `array[1] = new_value`.
// In the internal codes, we manually drop the value, and only mark Shared as combined in necessary.
impl Drop for Value {
    fn drop(&mut self) {
        if self.is_static() {
            return;
        }

        // optimize the drop overhead
        // when nodes been Combined and there may be inserted root node, we must traverse the tree
        if self.shared().is_combined() {
            self.drop_slow();
            return;
        }

        if self.is_root() {
            drop(self.arc_shared());
        } else {
            // If value is not root, it maybe dropped in place, and insert a new allocator in the document,
            // we mark Combined flag in the shared, to notify the root node to traverse the tree when dropping root.
            self.shared().set_combined()
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) union Meta {
    ptr: TaggedPtr<Shared>,
    val: u64,
}

pub(crate) const TYPE_MASK: u64 = (std::mem::align_of::<Shared>() as u64) - 1;
pub(crate) const ROOT_MASK: u64 = 0b1100;
pub(crate) const UNROOT_MASK: u64 = 0b1011;
/// shared ptr: | hi | valid ptr | tag |
pub(crate) const SHARED_PTR_MASK: u64 = 0x0000FFFFFFFFFFFF & !TYPE_MASK;
/// str ptr:    | hi | valid str ptr   |
pub(crate) const STR_PTR_MASK: u64 = 0x0000FFFFFFFFFFFF;

/// Encoding format:
/// static node
pub(crate) const NULL: u64 = 0b0000;
pub(crate) const FALSE: u64 = 0b0001;
pub(crate) const TRUE: u64 = 0b0010;
pub(crate) const _: u64 = 0b0011;
pub(crate) const FLOAT: u64 = 0b0100;
pub(crate) const UNSIGNED: u64 = 0b0101;
pub(crate) const SIGNED: u64 = 0b0110;
pub(crate) const _: u64 = 0b0111;
/// dynamic node
pub(crate) const STRING: u64 = 0b1000;
pub(crate) const _: u64 = 0b1001;
pub(crate) const ARRAY: u64 = 0b1010;
pub(crate) const OBJECT: u64 = 0b1011;
pub(crate) const ROOT_STRING: u64 = 0b1100;
pub(crate) const _: u64 = 0b1101;
pub(crate) const ROOT_ARRAY: u64 = 0b1110;
pub(crate) const ROOT_OBJECT: u64 = 0b1111;

impl Meta {
    pub(crate) const fn new(typ: u64, shared: *const Shared) -> Self {
        Self {
            ptr: TaggedPtr::new(shared, typ as usize),
        }
    }

    pub(crate) fn ptr(&self) -> *const Shared {
        unsafe { (self.val & SHARED_PTR_MASK) as *const _ }
    }

    pub(crate) fn set_ptr(&mut self, ptr: *const Shared) {
        unsafe {
            self.ptr.set_ptr(ptr);
        }
    }

    pub(crate) fn set_tag(&mut self, tag: u64) {
        unsafe {
            self.ptr.set_tag(tag as usize);
        }
    }

    pub(crate) fn tag(&self) -> u64 {
        unsafe { self.ptr.tag() as u64 }
    }
}

#[derive(Copy, Clone)]
pub(crate) union Data {
    pub(crate) uval: u64,
    pub(crate) ival: i64,
    pub(crate) fval: f64,
    pub(crate) sval: *const u8,
    pub(crate) achildren: *mut Value,
    pub(crate) ochildren: *mut Pair,
    pub(crate) parent: u64,
    pub(crate) info: NonNull<MetaNode>,
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe {
            match self.parent {
                0 => write!(f, "parent: null"),
                _ => write!(f, "parent: {}", self.parent),
            }
        }
    }
}

// Metanode is used to store the length and capacity of the array and object. and should be aligned as Values.
#[derive(Debug)]
pub(crate) struct MetaNode {
    len: u64,
    cap: u64,
    _pad: u64,
}

impl MetaNode {
    fn from_nodes(slice: &mut [Value]) -> &Self {
        debug_assert!(slice.len() >= Value::MEAT_NODE_COUNT);
        unsafe { &mut *(slice.as_mut_ptr() as *mut MetaNode) }
    }
}

thread_local! {
   static NODE_BUF: std::cell::RefCell<Vec<ManuallyDrop<Value>>> = std::cell::RefCell::new(Vec::new());
}

impl super::value_trait::JsonValueTrait for Value {
    type ValueType<'v> = &'v Value where Self: 'v;

    #[inline]
    fn get_type(&self) -> JsonType {
        match self.typ() {
            NULL => JsonType::Null,
            FALSE | TRUE => JsonType::Boolean,
            SIGNED | UNSIGNED | FLOAT => JsonType::Number,
            STRING | ROOT_STRING => JsonType::String,
            ARRAY | ROOT_ARRAY => JsonType::Array,
            OBJECT | ROOT_OBJECT => JsonType::Object,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn as_number(&self) -> Option<Number> {
        match self.typ() {
            UNSIGNED => self.as_u64().map(|u| u.into()),
            SIGNED => self.as_i64().map(|i| i.into()),
            FLOAT => self.as_f64().and_then(Number::from_f64),
            _ => None,
        }
    }

    #[inline]
    fn as_i64(&self) -> Option<i64> {
        match self.typ() {
            SIGNED => Some(self.i64()),
            UNSIGNED if self.u64() <= i64::MAX as u64 => Some(self.u64() as i64),
            _ => None,
        }
    }

    #[inline]
    fn as_u64(&self) -> Option<u64> {
        match self.typ() {
            UNSIGNED => Some(self.u64()),
            SIGNED if self.i64() >= 0 => Some(self.i64() as u64),
            _ => None,
        }
    }

    #[inline]
    fn as_f64(&self) -> Option<f64> {
        match self.typ() {
            UNSIGNED => Some(self.u64() as f64),
            SIGNED => Some(self.i64() as f64),
            FLOAT => Some(self.f64()),
            _ => None,
        }
    }

    #[inline]
    fn as_bool(&self) -> Option<bool> {
        match self.typ() {
            TRUE => Some(true),
            FALSE => Some(false),
            _ => None,
        }
    }

    #[inline]
    fn as_str(&self) -> Option<&str> {
        if self.is_str() {
            Some(self.str())
        } else {
            None
        }
    }

    #[inline]
    fn pointer<P: IntoIterator>(&self, path: P) -> Option<Self::ValueType<'_>>
    where
        P::Item: Index,
    {
        let path = path.into_iter();
        let mut value = self;
        for index in path {
            value = value.get(index)?;
        }
        Some(value)
    }

    #[inline]
    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        index.value_index_into(self)
    }
}

impl JsonContainerTrait for Value {
    type ArrayType = Array;
    type ObjectType = Object;

    #[inline]
    fn as_array(&self) -> Option<&Self::ArrayType> {
        if self.is_array() {
            Some(unsafe { transmute(self) })
        } else {
            None
        }
    }

    #[inline]
    fn as_object(&self) -> Option<&Self::ObjectType> {
        if self.is_object() {
            Some(unsafe { transmute(self) })
        } else {
            None
        }
    }
}

impl JsonValueMutTrait for Value {
    type ValueType = Value;
    type ArrayType = Array;
    type ObjectType = Object;

    #[inline]
    fn as_object_mut(&mut self) -> Option<&mut Self::ObjectType> {
        if self.is_object() {
            Some(unsafe { transmute(self) })
        } else {
            None
        }
    }

    #[inline]
    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType> {
        if self.is_array() {
            Some(unsafe { transmute(self) })
        } else {
            None
        }
    }

    #[inline]
    fn pointer_mut<P: IntoIterator>(&mut self, path: P) -> Option<&mut Self::ValueType>
    where
        P::Item: Index,
    {
        let mut path = path.into_iter();
        let mut value = self.get_mut(path.next().unwrap())?;
        for index in path {
            value = value.get_mut(index)?;
        }
        Some(value)
    }

    #[inline]
    fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self::ValueType> {
        index.index_into_mut(self)
    }
}

/// ValueRef is a immutable reference helper for Value.
///
/// # Example
///
/// ```
/// use sonic_rs::{ValueRef, json, JsonValueTrait};
///
/// let v = json!({
///    "name": "Xiaoming",
///    "age": 18,
/// });
///
/// match v.as_ref() {
///     ValueRef::Object(obj) => {
///        assert_eq!(obj.get(&"name").unwrap().as_str().unwrap(), "Xiaoming");
///        assert_eq!(obj.get(&"age").unwrap().as_i64().unwrap(), 18);
///    },
///    _ => unreachable!(),
/// }
/// ```
///
pub enum ValueRef<'a> {
    Null,
    Bool(bool),
    Number(Number),
    String(&'a str),
    Array(&'a Array),
    Object(&'a Object),
}

impl Value {
    const PADDING_SIZE: usize = 64;
    pub(crate) const MEAT_NODE_COUNT: usize = 1;

    /// Create a new `null` Value. It is also the default value of `Value`.
    ///
    #[inline]
    pub const fn new() -> Self {
        Value {
            // without shared allocator
            meta: Meta::new(NULL, std::ptr::null()),
            data: Data { uval: 0 },
        }
    }

    /// Create a reference `ValueRef` from a `&Value`.
    ///
    /// # Example
    ///
    /// ```
    /// use sonic_rs::{ValueRef, json, JsonValueTrait};
    ///
    /// let v = json!({
    ///    "name": "Xiaoming",
    ///    "age": 18,
    /// });
    ///
    /// match v.as_ref() {
    ///     ValueRef::Object(obj) => {
    ///        assert_eq!(obj.get(&"name").unwrap().as_str().unwrap(), "Xiaoming");
    ///        assert_eq!(obj.get(&"age").unwrap().as_i64().unwrap(), 18);
    ///    },
    ///    _ => unreachable!(),
    /// }
    /// ```
    ///
    #[inline]
    pub fn as_ref(&self) -> ValueRef<'_> {
        match self.typ() {
            NULL => ValueRef::Null,
            FALSE => ValueRef::Bool(false),
            TRUE => ValueRef::Bool(true),
            UNSIGNED => ValueRef::Number(self.as_u64().unwrap().into()),
            SIGNED => ValueRef::Number(self.as_i64().unwrap().into()),
            FLOAT => ValueRef::Number(Number::from_f64(self.as_f64().unwrap()).unwrap()),
            STRING | ROOT_STRING => ValueRef::String(self.as_str().unwrap()),
            ARRAY | ROOT_ARRAY => ValueRef::Array(self.as_array().unwrap()),
            OBJECT | ROOT_OBJECT => ValueRef::Object(self.as_object().unwrap()),
            _ => unreachable!(),
        }
    }

    /// Create a new string Value from a `&'static str` with zero-copy.
    ///
    #[inline]
    pub fn from_static_str(val: &'static str) -> Self {
        let mut v = Value {
            meta: Meta::new(STRING, std::ptr::null()),
            data: Data { sval: val.as_ptr() },
        };
        v.set_str_len(val.len());
        v
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_u64(val: u64, share: *const Shared) -> Self {
        Value {
            meta: Meta::new(UNSIGNED, share),
            data: Data { uval: val },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_in(share: Arc<Shared>) -> Self {
        let mut value = Value {
            meta: Meta::new(NULL, share.inner_ptr() as *const _),
            data: Data { uval: 0 },
        };
        value.mark_root();
        std::mem::forget(share);
        value
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_i64(val: i64, share: *const Shared) -> Self {
        Value {
            meta: Meta::new(SIGNED, share),
            data: Data { ival: val },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub(crate) unsafe fn new_f64_unchecked(val: f64, share: *const Shared) -> Self {
        Value {
            meta: Meta::new(FLOAT, share),
            data: Data { fval: val },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_f64(val: f64, share: *const Shared) -> Option<Self> {
        if val.is_finite() {
            Some(Value {
                meta: Meta::new(FLOAT, share),
                data: Data { fval: val },
            })
        } else {
            None
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_null(share: *const Shared) -> Self {
        Value {
            meta: Meta::new(NULL, share),
            data: Data { uval: 0 },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_array(share: *const Shared, capacity: usize) -> Self {
        let mut array = Value {
            meta: Meta::new(ARRAY, share),
            data: Data {
                achildren: std::ptr::null_mut(),
            },
        };
        if capacity == 0 {
            return array;
        }
        array.reserve::<Value>(capacity);
        array
    }

    #[inline(always)]
    fn at_pointer(&self, p: &PointerNode) -> Option<&Self> {
        match p {
            PointerNode::Key(key) => self.get_key(key),
            PointerNode::Index(index) => self.get_index(*index),
        }
    }

    #[inline(always)]
    fn at_pointer_mut(&mut self, p: &PointerNode) -> Option<&mut Self> {
        match p {
            PointerNode::Key(key) => self.get_key_mut(key).map(|v| v.0),
            PointerNode::Index(index) => self.get_index_mut(*index),
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_bool(val: bool, share: *const Shared) -> Self {
        Value {
            meta: Meta::new(if val { TRUE } else { FALSE }, share),
            data: Data { uval: 0 },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_str(val: &str, share: *const Shared) -> Self {
        let mut v = Value {
            meta: Meta::new(STRING, share),
            // TODO: optimize
            data: Data { sval: val.as_ptr() },
        };
        v.set_str_len(val.len());
        v
    }

    #[doc(hidden)]
    #[inline]
    pub fn copy_str(src: &str, share: &Shared) -> Self {
        let s = share.alloc.alloc_str(src);
        let mut v = Value {
            meta: Meta::new(STRING, share),
            data: Data { sval: s.as_ptr() },
        };
        v.set_str_len(s.len());
        v
    }

    // create a new owned allocator, and copied the string
    #[doc(hidden)]
    #[inline]
    pub fn new_str_owned<S: AsRef<str>>(src: S) -> Self {
        let shared = unsafe { &*Shared::new_ptr() };
        let s = shared.alloc.alloc_str(src.as_ref());
        let mut v = Value {
            meta: Meta::new(ROOT_STRING, shared),
            data: Data { sval: s.as_ptr() },
        };
        v.set_str_len(s.len());
        v
    }

    #[doc(hidden)]
    pub fn new_object(share: *const Shared, capacity: usize) -> Self {
        let mut object = Value {
            meta: Meta::new(OBJECT, share),
            data: Data {
                achildren: std::ptr::null_mut(),
            },
        };
        if capacity == 0 {
            return object;
        }
        object.reserve::<Pair>(capacity);
        object
    }

    pub(crate) fn check_shared(&mut self) -> &Shared {
        debug_assert!(self.is_container() || self.is_str());
        if self.is_static() {
            self.mark_shared(Shared::new_ptr());
        }
        self.shared()
    }

    pub(crate) fn shared(&self) -> &Shared {
        let addr = self.meta.ptr();
        debug_assert!(!addr.is_null(), "the ptr of Shared is null");
        debug_assert!((addr as usize) % 8 == 0, "the ptr of Shared is incorrect");
        unsafe { &*addr }
    }

    #[inline]
    pub(crate) fn arc_shared(&self) -> Arc<Shared> {
        let addr = self.meta.ptr();
        debug_assert!(!addr.is_null(), "the ptr of Shared is null");
        debug_assert!((addr as usize) % 8 == 0, "the ptr of Shared is incorrect");
        unsafe { Arc::from_raw(addr) }
    }

    #[inline]
    pub(crate) fn shared_clone(&self) -> Arc<Shared> {
        let addr = self.meta.ptr();
        debug_assert!(!addr.is_null(), "the ptr of Shared is null");
        debug_assert!((addr as usize) % 8 == 0, "the ptr of Shared is incorrect");
        unsafe { Arc::clone_from_raw(addr) }
    }

    /// node is flat, such as null, true, false, number and new empty array or object
    #[doc(hidden)]
    #[inline]
    pub fn is_static(&self) -> bool {
        self.meta.ptr().is_null()
    }

    pub(crate) fn is_container(&self) -> bool {
        self.is_array() || self.is_object()
    }

    #[doc(hidden)]
    #[inline]
    pub fn mark_shared(&mut self, shared: *const Shared) {
        self.meta.set_ptr(shared);
    }

    pub(crate) fn shared_parts(&self) -> *const Shared {
        self.meta.ptr()
    }

    unsafe fn raw_allocator(&self) -> &Bump {
        unsafe { &*self.shared().alloc.0.data_ptr() }
    }

    pub(crate) fn get_index(&self, index: usize) -> Option<&Self> {
        debug_assert!(self.is_array(), "{:?}", self);
        if let Some(s) = self.children::<Self>() {
            if index < s.len() {
                return Some(&s[index]);
            }
        }
        None
    }

    pub(crate) fn get_index_mut(&mut self, index: usize) -> Option<&mut Self> {
        debug_assert!(self.is_array());
        if let Some(s) = self.children_mut::<Self>() {
            if index < s.len() {
                return Some(&mut s[index]);
            }
        }
        None
    }

    #[inline]
    pub(crate) fn set_str_len(&mut self, len: usize) {
        // check length and the exisit ptr is valid
        unsafe {
            debug_assert!(len < crate::value::MAX_STR_SIZE);
            debug_assert!(self.meta.val >> 48 == 0);
            debug_assert!(self.data.uval >> 48 == 0);
            let hi = len >> 16;
            let lo = len & 0xFFFF;
            self.meta.val |= (hi as u64) << 48;
            self.data.uval |= (lo as u64) << 48;
        }
    }

    #[inline]
    pub(crate) fn get_key(&self, key: &str) -> Option<&Self> {
        self.get_key_value(key).map(|(_, v)| v)
    }

    pub(crate) fn get_key_value(&self, key: &str) -> Option<(&str, &Self)> {
        debug_assert!(self.is_object());
        if let Some(kv) = self.children::<(Self, Self)>() {
            for (k, v) in kv {
                let k = k.as_str().expect("key is not string");
                if k == key {
                    return Some((k, v));
                }
            }
        }
        None
    }

    pub(crate) fn children<T>(&self) -> Option<&[T]> {
        if self.has_children() {
            Some(self.children_unchecked::<T>())
        } else {
            None
        }
    }

    pub(crate) unsafe fn children_ptr<T>(&self) -> *const T {
        if self.has_children() {
            self.data.achildren.add(Self::MEAT_NODE_COUNT).cast()
        } else {
            NonNull::<T>::dangling().as_ptr()
        }
    }

    #[inline]
    pub(crate) unsafe fn children_mut_ptr<T>(&mut self) -> *mut T {
        if self.has_children() {
            self.data.achildren.add(Self::MEAT_NODE_COUNT).cast()
        } else {
            NonNull::<T>::dangling().as_ptr()
        }
    }

    #[inline]
    fn children_unchecked<T>(&self) -> &[T] {
        unsafe {
            let start = self.data.achildren.add(Self::MEAT_NODE_COUNT);
            let ptr = start as *const T;
            let len = self.len();
            from_raw_parts(ptr, len)
        }
    }

    #[inline]
    fn children_unchecked_mut<T>(&mut self) -> &mut [T] {
        unsafe {
            let start = self.data.achildren.add(Self::MEAT_NODE_COUNT);
            let ptr = start as *mut T;
            let len = self.len();
            from_raw_parts_mut(ptr, len)
        }
    }

    #[inline]
    pub(crate) fn get_key_offset(&self, key: &str) -> Option<usize> {
        debug_assert!(self.is_object());
        if let Some(kv) = self.children::<(Self, Self)>() {
            for (i, pair) in kv.iter().enumerate() {
                debug_assert!(pair.0.is_str());
                if pair.0.equal_str(key) {
                    return Some(i);
                }
            }
        }
        None
    }

    #[inline]
    pub(crate) fn get_key_mut(&mut self, key: &str) -> Option<(&mut Self, usize)> {
        if let Some(kv) = self.children_mut::<(Self, Self)>() {
            for (i, (k, v)) in kv.iter_mut().enumerate() {
                debug_assert!(k.is_str());
                if k.equal_str(key) {
                    return Some((v, i));
                }
            }
        }
        None
    }

    #[inline]
    pub(crate) fn insert_value(&mut self, index: usize, src: Value) {
        debug_assert!(self.is_array());
        self.reserve::<Value>(1);
        let children = self.children_mut_unchecked::<MaybeUninit<Value>>();
        let len = children.len();
        assert!(
            index <= children.len(),
            "index({}) should <= len({})",
            index,
            len
        );
        if index < len {
            unsafe {
                std::ptr::copy(
                    children.as_ptr().add(index),
                    children.as_mut_ptr().add(index + 1),
                    len - index,
                );
            }
        }
        unsafe {
            let dst = &mut *children.as_mut_ptr().add(index);
            write_value(dst, src, self.shared());
            self.add_len(1);
        }
    }

    #[inline]
    fn equal_str(&self, val: &str) -> bool {
        debug_assert!(self.is_str());
        self.str().len() == val.len() && self.str() == val
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        debug_assert!(self.is_object() || self.is_array());
        if self.has_children() {
            unsafe { self.data.info.as_ref().cap as usize }
        } else {
            0
        }
    }

    #[inline]
    fn allocator(&self) -> &SyncBump {
        &self.shared().alloc
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        debug_assert!(self.is_object() || self.is_array());

        if self.is_empty() {
            return;
        }

        // we need traverse the tree to drop the children
        if self.shared().is_combined() {
            if self.is_array() {
                for child in self.children_mut_unchecked::<Value>() {
                    child.drop_slow();
                }
            } else if self.is_object() {
                for child in self.children_mut_unchecked::<(Value, Value)>() {
                    child.0.drop_slow();
                    child.1.drop_slow();
                }
            }
        }
        unsafe { self.set_len(0) }
    }

    #[inline]
    pub(crate) fn remove_index(&mut self, index: usize) -> Value {
        debug_assert!(self.is_array());
        let children = self.children_mut_unchecked::<Value>();
        let len = children.len();
        assert!(
            index < len,
            "remove index({}) should be < len({})",
            index,
            len
        );
        let val = children[index].take();
        unsafe {
            std::ptr::copy_nonoverlapping(
                children.as_ptr().add(index + 1),
                children.as_mut_ptr().add(index),
                len - index - 1,
            );
        }
        self.add_len(-1);
        val
    }

    #[inline]
    pub(crate) fn remove_pair_index(&mut self, index: usize) -> (Value, Value) {
        debug_assert!(self.is_object());
        let children = self.children_mut_unchecked::<Pair>();
        let len = children.len();
        assert!(
            index < len,
            "remove index({}) should be < len({})",
            index,
            len
        );

        // key is always not a root, ignored it
        let children = self.children_mut_unchecked::<(Self, Self)>();
        // key will be dropped
        let key = children[index].0.take();
        let val = children[index].1.take();
        unsafe {
            let dst = children.as_mut_ptr().add(index);
            let src = children.as_ptr().add(index + 1);
            let size = len - index - 1;
            std::ptr::copy(src, dst, size);
        }
        self.add_len(-1);
        (key, val)
    }

    #[inline]
    pub(crate) fn remove_key(&mut self, k: &str) -> Option<Value> {
        debug_assert!(self.is_object());
        if let Some(i) = self.get_key_offset(k) {
            let (_, val) = self.remove_pair_index(i);
            Some(val)
        } else {
            None
        }
    }

    pub(crate) fn iter<T>(&self) -> std::slice::Iter<'_, T> {
        self.children::<T>().unwrap_or(&[]).iter()
    }

    pub(crate) fn iter_mut<T>(&mut self) -> std::slice::IterMut<'_, T> {
        self.children_mut::<T>().unwrap_or(&mut []).iter_mut()
    }

    /// Take the value from the node, and set the node as a empty node.
    /// Take will creat a new root node.
    ///
    /// # Examples
    /// ```
    /// use sonic_rs::json;
    /// use sonic_rs::JsonValueTrait;
    ///
    /// let mut value = json!({"a": 123});
    /// assert_eq!(value.take()["a"], 123);
    /// assert!(value.is_null());
    ///
    /// let mut value = json!(null);
    /// assert!(value.take().is_null());
    /// assert!(value.is_null());
    /// ```
    #[inline]
    pub fn take(&mut self) -> Self {
        replace_value(self, Value::default())
    }

    #[inline]
    pub(crate) unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(self.is_object() || self.is_array());
        let meta = unsafe { self.data.info.as_mut() };
        meta.len = len as u64;
    }

    #[inline]
    pub(crate) fn grow<T>(&mut self, capacity: usize) {
        if self.is_static() {
            self.mark_shared(Shared::new_ptr());
            self.mark_root();
        }
        let old = self.children::<T>();
        let nodes = capacity * (size_of::<T>() / size_of::<Value>()) + Self::MEAT_NODE_COUNT;
        let new_buffer: *mut Value = self.allocator().alloc_slice(nodes).as_mut_ptr();

        if let Some(children) = old {
            unsafe {
                let src = children.as_ptr();
                let dst: *mut T = new_buffer.add(Self::MEAT_NODE_COUNT).cast();
                std::ptr::copy_nonoverlapping(src, dst, children.len());
            }
        }

        // set the capacity and length
        let first: &mut MetaNode = unsafe { &mut *new_buffer.cast() };
        first.cap = capacity as u64;
        first.len = old.map_or(0, |s| s.len()) as u64;
        self.data.achildren = new_buffer.cast();
    }

    #[inline]
    pub(crate) fn reserve<T>(&mut self, additional: usize) {
        debug_assert!(self.is_object() || self.is_array());
        debug_assert!(size_of::<T>() == size_of::<Value>() || size_of::<T>() == size_of::<Pair>());

        let cur_cap = self.capacity();
        let required_cap = self
            .len()
            .checked_add(additional)
            .expect("capacity overflow");
        let default_cap = if size_of::<T>() == size_of::<Value>() {
            super::array::DEFAULT_ARRAY_CAP
        } else {
            super::object::DEFAULT_OBJ_CAP
        };

        if required_cap > self.capacity() {
            let cap = std::cmp::max(cur_cap * 2, required_cap);
            let cap = std::cmp::max(default_cap, cap);
            self.grow::<T>(cap);
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn append_value(&mut self, val: Value) -> &mut Value {
        debug_assert!(self.is_array());
        self.reserve::<Value>(1);

        let children = self.children_mut_unchecked::<MaybeUninit<Value>>();
        let len = children.len();
        let end = unsafe { &mut *children.as_mut_ptr().add(len) };
        write_value(end, val, self.shared());
        self.add_len(1);
        let ret = unsafe { end.assume_init_mut() };
        ret
    }

    #[doc(hidden)]
    #[inline]
    pub fn append_pair(&mut self, pair: Pair) -> &mut Pair {
        debug_assert!(self.is_object());
        self.reserve::<Pair>(1);

        let children = self.children_mut_unchecked::<(MaybeUninit<Value>, MaybeUninit<Value>)>();
        let len = children.len();

        let end_key = unsafe { &mut (*children.as_mut_ptr().add(len)).0 };
        let end_value = unsafe { &mut (*children.as_mut_ptr().add(len)).1 };
        write_value(end_key, pair.0, self.shared());
        write_value(end_value, pair.1, self.shared());
        self.add_len(1);
        unsafe { &mut *(end_key as *mut _ as *mut Pair) }
    }

    fn add_len(&mut self, additional: isize) {
        debug_assert!(self.is_array() || self.is_object());
        let meta = unsafe { self.data.info.as_mut() };
        meta.len = (meta.len as isize + additional) as u64;
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<Value> {
        debug_assert!(self.is_array());
        if self.is_empty() {
            return None;
        }

        let children = self.children_mut_unchecked::<Value>();
        let len = children.len();
        let val = children[len - 1].take();
        self.add_len(-1);
        Some(val)
    }

    #[inline]
    pub(crate) fn pop_pair(&mut self) -> Option<Pair> {
        debug_assert!(self.is_object());
        if self.is_empty() {
            return None;
        }

        let children = self.children_mut_unchecked::<Pair>();
        let len = children.len();
        let pair = (children[len - 1].0.take(), children[len - 1].1.take());
        self.add_len(-1);
        Some(pair)
    }

    #[inline]
    fn has_children(&self) -> bool {
        unsafe { self.data.achildren as usize != 0 }
    }

    #[inline]
    pub(crate) fn children_mut<T>(&mut self) -> Option<&mut [T]> {
        if self.has_children() {
            Some(self.children_mut_unchecked::<T>())
        } else {
            None
        }
    }

    #[inline]
    fn children_mut_unchecked<T>(&mut self) -> &mut [T] {
        unsafe {
            let start = self.data.achildren.add(Self::MEAT_NODE_COUNT);
            let ptr = start as *mut T;
            let len = self.len();
            from_raw_parts_mut(ptr, len)
        }
    }

    #[inline(never)]
    pub(crate) fn parse_with_padding(&mut self, json: &[u8]) -> Result<usize> {
        let alloc = unsafe { self.raw_allocator() };
        let len = json.len();

        // allocate the padding buffer for the input json
        let real_size = len + Self::PADDING_SIZE;
        let layout = Layout::array::<u8>(real_size).map_err(Error::custom)?;
        let dst = alloc.alloc_layout(layout);
        let json_buf = unsafe {
            let dst = dst.as_ptr();
            std::ptr::copy_nonoverlapping(json.as_ptr(), dst, len);
            // fix miri warnings, actual this code can be removed because we set a guard for the json
            std::ptr::write_bytes(dst.add(len), 0, Self::PADDING_SIZE);
            *(dst.add(len)) = b'x';
            *(dst.add(len + 1)) = b'"';
            *(dst.add(len + 2)) = b'x';
            std::slice::from_raw_parts_mut(dst, len + Self::PADDING_SIZE)
        };
        let slice = PaddedSliceRead::new(json_buf);
        let mut parser = Parser::new(slice);

        // optimize: use a pre-allocated vec.
        // If json is valid, the max number of value nodes should be
        // half of the valid json length + 2. like as [1,2,3,1,2,3...]
        // if the capacity is not enough, we will return a error.
        let nodes = NODE_BUF.with(|buf| {
            let mut nodes = buf.borrow_mut();
            nodes.clear();
            nodes.reserve((json.len() / 2) + 2);
            unsafe {
                let ptr = (&mut *nodes) as *mut Vec<ManuallyDrop<Value>>;
                &mut *ptr
            }
        });

        let mut visitor = DocumentVisitor {
            shared: unsafe { &*(self.shared() as *const Shared) },
            nodes: nodes.into(),
            parent: 0,
        };
        parser.parse_value_with_padding(&mut visitor)?;
        self.data = visitor.nodes()[0].data;
        self.meta = visitor.nodes()[0].meta;
        self.mark_root();
        Ok(parser.read.index())
    }

    #[inline(never)]
    pub(crate) fn parse_without_padding<'de, R: Reader<'de>>(
        &mut self,
        parser: &mut Parser<R>,
    ) -> Result<()> {
        let remain_len = parser.read.remain();
        let nodes = NODE_BUF.with(|buf| {
            let mut nodes = buf.borrow_mut();
            nodes.clear();
            nodes.reserve((remain_len / 2) + 2);
            unsafe {
                let ptr = (&mut *nodes) as *mut Vec<ManuallyDrop<Value>>;
                &mut *ptr
            }
        });

        let mut visitor = DocumentVisitor {
            shared: unsafe { &*(self.shared() as *const Shared) },
            nodes: nodes.into(),
            parent: 0,
        };
        parser.parse_value_without_padding(&mut visitor)?;
        self.data = visitor.nodes()[0].data;
        self.meta = visitor.nodes()[0].meta;
        self.mark_root();
        Ok(())
    }

    fn typ(&self) -> u64 {
        self.meta.tag()
    }

    fn i64(&self) -> i64 {
        unsafe { self.data.ival }
    }

    fn u64(&self) -> u64 {
        unsafe { self.data.uval }
    }

    fn f64(&self) -> f64 {
        unsafe { self.data.fval }
    }

    fn str(&self) -> &str {
        unsafe {
            let ptr = (self.data.uval & STR_PTR_MASK) as *const u8;
            let len = self.str_len();
            let slice = std::slice::from_raw_parts(ptr, len);
            from_utf8_unchecked(slice)
        }
    }

    pub(crate) fn str_len(&self) -> usize {
        debug_assert!(self.is_str());
        unsafe {
            let hi = (self.meta.val >> 48) as usize;
            let lo = (self.data.uval >> 48) as usize;
            hi << 16 | lo
        }
    }

    pub(crate) fn len(&self) -> usize {
        unsafe {
            if (self.data.achildren as usize) == 0 {
                return 0;
            }
            self.data.info.as_ref().len as usize
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn array(&self) -> &[Value] {
        if self.is_empty() {
            return &[];
        }
        unsafe {
            let children = self.data.achildren;
            let meta = &*(children as *const MetaNode);
            from_raw_parts(children.add(Value::MEAT_NODE_COUNT), meta.len as usize)
        }
    }

    fn object(&self) -> &[(Value, Value)] {
        if self.is_empty() {
            return &[];
        }
        unsafe {
            let children = self.data.achildren;
            let meta = &*(children as *const MetaNode);
            from_raw_parts(
                children.add(Value::MEAT_NODE_COUNT) as *mut Pair,
                meta.len as usize,
            )
        }
    }

    #[inline]
    pub(crate) fn state(&mut self) -> ValueState<'_> {
        if self.is_static() {
            ValueState::Static(self)
        } else if self.is_root() {
            ValueState::Root(self)
        } else if self.is_inlined() {
            ValueState::Inlined(self)
        } else {
            ValueState::Shared(self)
        }
    }
}

pub(crate) enum ValueState<'a> {
    // Value without a shared allocator
    Static(&'a mut Value),
    // Value with a share allocator
    Shared(&'a mut Value),
    // Value is root and with a owned allocator
    Root(&'a mut Value),
    // Value is static but is a children and with shared allocator ptr
    Inlined(&'a mut Value),
}

#[derive(Default, Debug)]
pub(crate) struct OwnedValue(Value);

impl From<OwnedValue> for Value {
    #[inline]
    fn from(v: OwnedValue) -> Self {
        v.0
    }
}

// Replace dst with a new `OwnedValue`, and return the old `Value` as a `OwnedValue`.
#[inline]
pub(crate) fn replace_value(dst: &mut Value, mut src: Value) -> Value {
    match dst.state() {
        ValueState::Static(dst) => {
            let old = std::mem::replace(dst, src);
            return old;
        }
        ValueState::Shared(_) | ValueState::Inlined(_) => {}
        ValueState::Root(dst) => return std::mem::replace(dst, src),
    }

    let dst_shared = dst.shared();
    match src.state() {
        ValueState::Static(src) | ValueState::Inlined(src) => {
            src.mark_shared(dst_shared);
        }
        ValueState::Root(src) => {
            if std::ptr::eq(src.shared_parts(), dst_shared) {
                drop(src.arc_shared());
                src.unmark_root();
            } else {
                dst_shared.set_combined();
            }
        }
        ValueState::Shared(_) => unreachable!("should not be shared"),
    }

    // make old from `Shared` into `Owned`
    let mut old = std::mem::replace(dst, src);
    old.mark_root();
    if old.is_root() {
        std::mem::forget(old.shared_clone());
    }
    old
}

// Write dst with a new `OwnedValue`. The dst is a uninitialized value and should not be drop.
// The uninitialized value is allocated in the `shared` allocator.
#[inline]
pub(crate) fn write_value(dst: &mut MaybeUninit<Value>, mut src: Value, shared: &Shared) {
    match src.state() {
        ValueState::Static(sv) => {
            sv.mark_shared(shared);
            dst.write(src);
        }
        ValueState::Root(sv) => {
            if std::ptr::eq(sv.shared_parts(), shared) {
                sv.unmark_root();
                drop(sv.arc_shared());
            } else {
                shared.set_combined();
            }
            dst.write(src);
        }
        ValueState::Shared(sv) | ValueState::Inlined(sv) => {
            assert!(
                std::ptr::eq(sv.shared_parts(), shared),
                "should be same allocator"
            );
            dst.write(src);
        }
    }
}

// a simple wrapper for visitor
pub(crate) struct DocumentVisitor<'a> {
    pub(crate) shared: &'a Shared,
    pub(crate) nodes: NonNull<Vec<ManuallyDrop<Value>>>,
    pub(crate) parent: usize,
}

impl<'a> DocumentVisitor<'a> {
    // the array and object's logic is same.
    fn visit_container(&mut self, len: usize) -> bool {
        let visitor = self;
        let alloc = unsafe { &*visitor.shared.alloc.0.data_ptr() };
        let parent = visitor.parent;
        let old = unsafe { visitor.nodes()[parent].data.parent as usize };
        visitor.parent = old;
        if len == 0 {
            let container = &mut visitor.nodes()[parent];
            container.data.achildren = std::ptr::null_mut();
            return true;
        }
        unsafe {
            let visited_children = &visitor.nodes()[(parent + 1)..];
            let real_count = visited_children.len() + Value::MEAT_NODE_COUNT;
            let layout = {
                if let Ok(layout) = Layout::array::<Value>(real_count) {
                    layout
                } else {
                    return false;
                }
            };
            let mut children = alloc.alloc_layout(layout);
            // copy visited nodes into document
            let src = visited_children.as_ptr();
            let dst = children.as_ptr() as *mut ManuallyDrop<Value>;
            let dst = dst.add(Value::MEAT_NODE_COUNT);
            std::ptr::copy_nonoverlapping(src, dst, visited_children.len());

            // set the capacity and length
            let meta = &mut *(children.as_mut() as *mut _ as *mut MetaNode);
            meta.cap = len as u64;
            meta.len = len as u64;
            let container = &mut visitor.nodes()[parent];
            container.data.achildren = children.as_mut() as *mut _ as *mut Value;
            // must reset the length, because we copy the children into bumps
            visitor.nodes().set_len(parent + 1);
        }
        true
    }

    #[inline(always)]
    fn push_node(&mut self, node: Value) -> bool {
        if self.nodes().len() == self.nodes().capacity() {
            false
        } else {
            self.nodes().push(ManuallyDrop::new(node));
            true
        }
    }

    #[inline(always)]
    fn shared(&self) -> *const Shared {
        self.shared
    }

    #[inline(always)]
    fn nodes(&mut self) -> &mut Vec<ManuallyDrop<Value>> {
        unsafe { self.nodes.as_mut() }
    }
}

impl<'de, 'a: 'de> JsonVisitor<'de> for DocumentVisitor<'a> {
    #[inline(always)]
    fn visit_bool(&mut self, val: bool) -> bool {
        self.push_node(Value::new_bool(val, self.shared as *const _))
    }

    #[inline(always)]
    fn visit_f64(&mut self, val: f64) -> bool {
        // # Safety
        // we have checked the f64 in parsing number.
        let node = unsafe { Value::new_f64_unchecked(val, self.shared as *const _) };
        self.push_node(node)
    }

    #[inline(always)]
    fn visit_i64(&mut self, val: i64) -> bool {
        self.push_node(Value::new_i64(val, self.shared as *const _))
    }

    #[inline(always)]
    fn visit_u64(&mut self, val: u64) -> bool {
        self.push_node(Value::new_u64(val, self.shared as *const _))
    }

    #[inline(always)]
    fn visit_array_start(&mut self, _hint: usize) -> bool {
        let ret = self.push_node(Value::new_array(self.shared as *const _, 0));
        // record the parent container position
        let len = self.nodes().len();
        self.nodes()[len - 1].data.parent = self.parent as u64;
        self.parent = len - 1;
        ret
    }

    #[inline(always)]
    fn visit_array_end(&mut self, len: usize) -> bool {
        self.visit_container(len)
    }

    #[inline(always)]
    fn visit_object_start(&mut self, _hint: usize) -> bool {
        let ret = self.push_node(Value::new_object(self.shared as *const _, 0));
        let len = self.nodes().len();
        self.nodes()[len - 1].data.parent = self.parent as u64;
        self.parent = len - 1;
        ret
    }

    #[inline(always)]
    fn visit_object_end(&mut self, len: usize) -> bool {
        self.visit_container(len)
    }

    #[inline(always)]
    fn visit_null(&mut self) -> bool {
        self.push_node(Value::new_null(self.shared as *const _))
    }

    // this api should never used for parsing with padding buffer
    #[inline(always)]
    fn visit_str(&mut self, value: &str) -> bool {
        let alloc = unsafe { &*self.shared.alloc.0.data_ptr() };
        let value = alloc.alloc_str(value);
        self.push_node(Value::new_str(value, self.shared as *const _))
    }

    #[inline(always)]
    fn visit_borrowed_str(&mut self, value: &'de str) -> bool {
        self.push_node(Value::new_str(value, self.shared as *const _))
    }

    #[inline(always)]
    fn visit_key(&mut self, key: &str) -> bool {
        self.visit_str(key)
    }

    #[inline(always)]
    fn visit_borrowed_key(&mut self, key: &'de str) -> bool {
        self.visit_borrowed_str(key)
    }
}

impl Serialize for Value {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        match self.typ() {
            NULL => serializer.serialize_unit(),
            TRUE => serializer.serialize_bool(true),
            FALSE => serializer.serialize_bool(false),
            SIGNED => serializer.serialize_i64(self.i64()),
            UNSIGNED => serializer.serialize_u64(self.u64()),
            FLOAT => serializer.serialize_f64(self.f64()),
            STRING | ROOT_STRING => serializer.serialize_str(self.str()),
            ARRAY | ROOT_ARRAY => {
                let nodes = self.array();
                let mut seq = tri!(serializer.serialize_seq(Some(nodes.len())));
                for n in nodes {
                    tri!(seq.serialize_element(n));
                }
                seq.end()
            }
            OBJECT | ROOT_OBJECT => {
                let entrys = self.object();
                let mut map = tri!(serializer.serialize_map(Some(entrys.len())));
                for (k, v) in entrys {
                    tri!(map.serialize_entry(k, v));
                }
                map.end()
            }
            _ => panic!("unsupported types"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::from_slice;
    use crate::{
        error::{make_error, Result},
        pointer,
    };
    use std::path::Path;

    fn test_value(data: &str) -> Result<()> {
        let serde_value: serde_json::Result<serde_json::Value> = serde_json::from_str(data);
        let dom: Result<Value> = from_slice(data.as_bytes());
        if let Ok(serde_value) = serde_value {
            let dom = dom.unwrap();
            let sonic_out = crate::to_string(&dom)?;
            let serde_value2: serde_json::Value = serde_json::from_str(&sonic_out).unwrap();
            if serde_value == serde_value2 {
                Ok(())
            } else {
                diff_json(data);
                Err(make_error(format!(
                    "invalid result for valid json {}",
                    data
                )))
            }
        } else {
            if dom.is_err() {
                return Ok(());
            }
            let dom = dom.unwrap();
            Err(make_error(format!(
                "invalid result for invalid json {}, got {}",
                data,
                crate::to_string(&dom).unwrap(),
            )))
        }
    }

    fn diff_json(data: &str) {
        let serde_value: serde_json::Value = serde_json::from_str(data).unwrap();
        let dom: Value = from_slice(data.as_bytes()).unwrap();
        let sonic_out = crate::to_string(&dom).unwrap();
        let serde_value2: serde_json::Value = serde_json::from_str(&sonic_out).unwrap();
        let expect = serde_json::to_string_pretty(&serde_value).unwrap();
        let got = serde_json::to_string_pretty(&serde_value2).unwrap();

        fn write_to(file: &str, data: &str) -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::File::create(file)?;
            file.write_all(data.as_bytes())?;
            Ok(())
        }

        if serde_value != serde_value2 {
            write_to("got.json", &got).unwrap();
            write_to("expect.json", &expect).unwrap();
        }
    }

    fn test_value_file(path: &Path) {
        let data = std::fs::read_to_string(path).unwrap();
        assert!(test_value(&data).is_ok(), "failed json is  {:?}", path);
    }

    #[test]
    fn test_node_basic() {
        // Valid JSON object
        let data = r#"{"name": "John", "age": 30}"#;
        assert!(test_value(data).is_ok(), "failed json is {}", data);

        // Valid JSON array
        let data = r#"[1, 2, 3]"#;
        assert!(test_value(data).is_ok(), "failed json is {}", data);

        // Valid JSON data with nested objects and arrays
        let data = r#"{
            "name": "John",
            "age": 30,
            "cars": [
                { "name": "Ford", "models": ["Fiesta", "Focus", "Mustang"] },
                { "name": "BMW", "models": ["320", "X3", "X5"] },
                { "name": "Fiat", "models": ["500", "Panda"] }
            ],
            "address": {
                "street": "Main Street",
                "city": "New York",
                "state": "NY",
                "zip": "10001"
            }
        }"#;
        assert!(test_value(data).is_ok(), "failed json is {}", data);

        // Valid JSON data with escape characters
        let data = r#"{
            "name": "John",
            "age": 30,
            "description": "He said, \"I'm coming home.\""
        }"#;
        assert!(test_value(data).is_ok(), "failed json is {}", data);
    }

    #[test]
    fn test_node_from_files3() {
        use std::fs::DirEntry;
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/benches/testdata/";
        println!("dir is {}", path);

        let mut files: Vec<DirEntry> = std::fs::read_dir(path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().ok().map(|t| t.is_file()).unwrap_or(false))
            .collect();

        files.sort_by(|a, b| {
            a.metadata()
                .unwrap()
                .len()
                .cmp(&b.metadata().unwrap().len())
        });

        for file in files {
            let path = file.path();
            if path.extension().unwrap_or_default() == "json" && !path.ends_with("canada.json") {
                println!(
                    "test json file: {:?},  {} bytes",
                    path,
                    file.metadata().unwrap().len()
                );
                test_value_file(&path)
            }
        }
    }

    #[test]
    fn test_json_tralings() {
        let testdata = [
            "-0.99999999999999999xxx",
            "\"\"\"",
            "{} x",
            "\"xxxxx",
            r#""\uDBDD\u1DD000"#,
        ];

        for data in testdata {
            let ret: Result<Value> = from_slice(data.as_bytes());
            assert!(ret.is_err(), "failed json is {}", data);
        }
    }

    #[test]
    fn test_parse_numbrs() {
        let testdata = [
            " 33.3333333043333333",
            " 33.3333333043333333 ",
            " 33.3333333043333333--",
            &f64::MAX.to_string(),
            &f64::MIN.to_string(),
            &u64::MAX.to_string(),
            &u64::MIN.to_string(),
            &i64::MIN.to_string(),
            &i64::MAX.to_string(),
        ];
        for data in testdata {
            test_value(data).unwrap();
        }
    }

    #[test]
    fn test_parse_escaped() {
        let testdata = [
            r#""\\9,\ud9CC\u8888|""#,
            r#"{"\t:0000000006427[{\t:003E:[[{0.77":96}"#,
        ];
        for data in testdata {
            test_value(data).unwrap();
        }
    }
    const TEST_JSON: &str = r#"{
        "bool": true,
        "int": -1,
        "uint": 0,
        "float": 1.1,
        "string": "hello",
        "array": [1,2,3],
        "object": {"a":"aaa"},
        "strempty": "",
        "objempty": {},
        "arrempty": []
    }"#;

    #[test]
    fn test_value_is() {
        let value: Value = crate::from_str(TEST_JSON).unwrap();
        assert!(value.get("bool").is_boolean());
        assert!(value.get("bool").is_true());
        assert!(value.get("uint").is_u64());
        assert!(value.get("uint").is_number());
        assert!(value.get("int").is_i64());
        assert!(value.get("float").is_f64());
        assert!(value.get("string").is_str());
        assert!(value.get("array").is_array());
        assert!(value.get("object").is_object());
        assert!(value.get("strempty").is_str());
        assert!(value.get("objempty").is_object());
        assert!(value.get("arrempty").is_array());
    }

    #[test]
    fn test_value_get() {
        let value: Value = crate::from_str(TEST_JSON).unwrap();
        assert_eq!(value.get("int").as_i64().unwrap(), -1);
        assert_eq!(value["array"].get(0).as_i64().unwrap(), 1);

        assert_eq!(value.pointer(&pointer!["array", 2]).as_u64().unwrap(), 3);
        assert_eq!(
            value.pointer(&pointer!["object", "a"]).as_str().unwrap(),
            "aaa"
        );
        assert_eq!(value.pointer(&pointer!["objempty", "a"]).as_str(), None);

        assert_eq!(value.pointer(&pointer!["arrempty", 1]).as_str(), None);

        assert!(!value.pointer(&pointer!["unknown"]).is_str());
    }

    #[test]
    fn test_invalid_utf8() {
        use crate::from_slice;
        use crate::from_slice_unchecked;

        let data = [b'"', 0x80, 0x90, b'"'];
        let ret: Result<Value> = from_slice(&data);
        assert_eq!(
            ret.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 1\n\n\t\"��\"\n\t.^..\n"
        );

        let dom: Result<Value> = unsafe { from_slice_unchecked(&data) };
        assert!(dom.is_ok(), "{}", dom.unwrap_err());

        let data = [b'"', b'"', 0x80];
        let dom: Result<Value> = from_slice(&data);
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 2\n\n\t\"\"�\n\t..^\n"
        );

        let data = [0x80, b'"', b'"'];
        let dom: Result<Value> = unsafe { from_slice_unchecked(&data) };
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid JSON value at line 1 column 0\n\n\t�\"\"\n\t^..\n"
        );
    }

    #[test]
    fn test_value_serde() {
        use crate::{array, object};
        use serde::Deserialize;
        use serde::Serialize;
        #[derive(Deserialize, Debug, Serialize, PartialEq)]
        struct Foo {
            value: Value,
            object: Object,
            array: Array,
        }

        let foo: Foo = crate::from_str(
            r#"
        {
            "value": "hello",
            "object": {"a": "b"},
            "array": [1,2,3]
        }"#,
        )
        .unwrap();

        assert_eq!(ManuallyDrop::new(foo.value.arc_shared()).refcnt(), 3);
        assert_eq!(
            foo,
            Foo {
                value: Value::from("hello"),
                object: object! {"a": "b"},
                array: array![1, 2, 3],
            }
        );

        let _ = crate::from_str::<Foo>(
            r#"{
                "value": "hello",
                "object": {"a": "b"},
                "array": [1,2,3
            }"#,
        )
        .unwrap_err();
    }
}
