use super::value_trait::{JsonType, JsonValue};
use super::Index;
use super::IndexMut;
use crate::error::make_error;
use crate::error::Result;
use crate::parser::Parser;
use crate::pointer::{JsonPointer, PointerNode};
use crate::reader::UncheckedSliceRead;
use crate::serde::tri;
use crate::util::utf8::from_utf8;
use crate::visitor::JsonVisitor;
use crate::{to_string, Number};
use bumpalo::Bump;
use core::mem::size_of;
use serde::ser::{Error, Serialize, SerializeMap, SerializeSeq};
use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops;
use std::ptr::NonNull;
use std::slice::{from_raw_parts, from_raw_parts_mut};

/// Value is a node in the DOM tree.
pub struct Value<'dom> {
    typ: NodeMeta,
    val: NodeValue<'dom>,
}

impl<'dom> Default for Value<'dom> {
    fn default() -> Self {
        Self::new_uinit()
    }
}

impl From<bool> for Value<'_> {
    fn from(val: bool) -> Self {
        Self::new_bool(val)
    }
}

impl From<u64> for Value<'_> {
    fn from(val: u64) -> Self {
        Self::new_u64(val)
    }
}

impl From<i64> for Value<'_> {
    fn from(val: i64) -> Self {
        Self::new_i64(val)
    }
}

impl TryFrom<f64> for Value<'_> {
    type Error = crate::Error;

    fn try_from(value: f64) -> std::result::Result<Self, Self::Error> {
        Self::new_f64(value)
            .ok_or_else(|| make_error("NaN or Infinity is not a valid JSON value".to_string()))
    }
}

impl<'dom> std::fmt::Debug for Value<'dom> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(to_string(self).unwrap_or_else(|e| e.to_string()).as_str())
    }
}

/// Object is a JSON object.
#[derive(Debug, Copy, Clone)]
pub struct Object<'dom>(&'dom Value<'dom>);

/// ObjectMut is a mutable JSON object.
#[derive(Debug)]
pub struct ObjectMut<'dom>(ValueMut<'dom>);

impl<'dom> Object<'dom> {
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn get(&self, key: &str) -> Option<&Value<'dom>> {
        self.0.get_key(key)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'dom> ObjectMut<'dom> {
    pub fn allocator(&self) -> &'dom Bump {
        self.0.alloc
    }

    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.as_ref().capacity()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.as_ref().contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    pub fn get(&'dom self, key: &str) -> Option<&Value<'dom>> {
        self.0.get(key)
    }

    pub fn get_mut(&'dom mut self, key: &str) -> Option<ValueMut<'dom>> {
        if let Some(node) = self.0.val.get_mut(key) {
            Some(ValueMut {
                val: node,
                alloc: self.0.alloc,
            })
        } else {
            None
        }
    }

    // return old value if the k is exitsted
    pub fn insert(&mut self, k: &str, v: Value<'dom>) -> Option<Value<'dom>> {
        if let Some(node) = self.0.val.get_mut(k) {
            let old = node.take();
            *(node) = v;
            Some(old)
        } else {
            self.0
                .val
                .append_object((Value::new_str(k, self.0.alloc), v), self.0.alloc);
            None
        }
    }

    pub fn remove(&mut self, k: &str) -> Option<Value<'dom>> {
        self.0.val.remove_in_object(k)
    }

    pub fn pop(&mut self) -> Option<Value<'dom>> {
        if self.is_empty() {
            None
        } else {
            let children = self.0.val.children_mut_unchecked::<Value>();
            let node = children[children.len() - 1].take();
            self.0.val.add_len(-1);
            Some(node)
        }
    }

    pub fn reserve(&mut self, addtional: usize) {
        self.0.val.reserve_object(addtional, self.0.alloc);
    }

    // as_ref only used in internal, so safe here.
    fn as_ref(&self) -> Object<'dom> {
        unsafe { *(self as *const Self as *const Object<'dom>) }
    }
}

/// Array is a JSON array.
#[derive(Debug, Copy, Clone)]
pub struct Array<'dom>(&'dom Value<'dom>);

/// ArrayMut is a mutable JSON array.
pub struct ArrayMut<'dom>(ValueMut<'dom>);

impl<'dom> Array<'dom> {
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'dom> ops::Deref for Array<'dom> {
    type Target = [Value<'dom>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.children_unchecked()
    }
}

impl<'dom> ArrayMut<'dom> {
    pub fn push(&mut self, node: Value<'dom>) {
        self.0.val.append_array(node, self.0.alloc)
    }

    pub fn pop(&mut self) -> Option<Value<'dom>> {
        if self.is_empty() {
            None
        } else {
            let children = self.0.val.children_mut_unchecked::<Value>();
            let node = children[children.len() - 1].take();
            self.0.val.add_len(-1);
            Some(node)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    pub fn capacity(&self) -> usize {
        self.as_ref().capacity()
    }

    pub fn reserve(&mut self, addtional: usize) {
        self.0.val.reserve_array(addtional, self.0.alloc);
    }

    pub fn allocator(&self) -> &'dom Bump {
        self.0.alloc
    }

    // as_ref only used in internal, so safe here.
    fn as_ref(&self) -> Array<'dom> {
        unsafe { *(self as *const Self as *const Array<'dom>) }
    }
}

impl<'dom> ops::Deref for ArrayMut<'dom> {
    type Target = [Value<'dom>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.val.children_unchecked()
    }
}

impl<'dom> ops::DerefMut for ArrayMut<'dom> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [Value<'dom>] {
        self.0.val.children_mut_unchecked()
    }
}

#[derive(Debug, Copy, Clone)]
struct NodeMeta(u64);

#[derive(Copy, Clone)]
union NodeValue<'dom> {
    uval: u64,
    ival: i64,
    fval: f64,
    parent: u64,
    nptr: NonNull<Value<'dom>>,
    first: NonNull<MetaNode>,

    str_ptr: *const u8,
    own_ptr: *mut u8,
    _lifetime: PhantomData<&'dom Document>,
}

impl<'a> JsonValue for Value<'a> {
    type ValueType<'dom> = &'dom Value<'dom> where Self: 'dom;

    fn get_type(&self) -> JsonType {
        const TYPE_MASK: u64 = 0b111;
        ((self.typ.0 & TYPE_MASK) as u8).into()
    }

    fn as_number(&self) -> Option<Number> {
        match self.typ() {
            Value::UNSIGNED => self.as_u64().map(|u| u.into()),
            Value::SIGNED => self.as_i64().map(|i| i.into()),
            Value::FLOAT => self.as_f64().and_then(Number::from_f64),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self.typ() {
            Value::SIGNED => Some(self.i64()),
            Value::UNSIGNED if self.u64() <= i64::MAX as u64 => Some(self.u64() as i64),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self.typ() {
            Value::UNSIGNED => Some(self.u64()),
            Value::SIGNED if self.i64() >= 0 => Some(self.i64() as u64),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.typ() {
            Value::UNSIGNED => Some(self.u64() as f64),
            Value::SIGNED => Some(self.i64() as f64),
            Value::FLOAT => Some(self.f64()),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self.typ() {
            Value::TRUE => Some(true),
            Value::FALSE => Some(false),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        if self.typ() == Value::STRING {
            Some(self.str())
        } else {
            None
        }
    }

    fn pointer<'dom>(&'dom self, path: &JsonPointer) -> Option<Self::ValueType<'dom>> {
        let mut node = self;
        for p in path {
            if let Some(next) = node.at_pointer(p) {
                node = next;
            } else {
                return None;
            }
        }
        Some(node)
    }

    fn get<'dom, I: Index>(&'_ self, index: I) -> Option<Self::ValueType<'_>> {
        index.value_index_into(self)
    }
}

impl<'dom> Value<'dom> {
    const NULL: u64 = (JsonType::Null as u64);
    const FALSE: u64 = (JsonType::Boolean as u64);
    const TRUE: u64 = (JsonType::Boolean as u64) | (1 << 3);
    const UNSIGNED: u64 = (JsonType::Number as u64);
    const SIGNED: u64 = (JsonType::Number as u64) | (1 << 3);
    const FLOAT: u64 = (JsonType::Number as u64) | (2 << 3);
    const ARRAY: u64 = (JsonType::Array as u64);
    const OBJECT: u64 = (JsonType::Object as u64);
    const STRING: u64 = (JsonType::String as u64);

    // type bits
    const LEN_BITS: u64 = 8;

    // the count of meta node
    // layout as:
    // [ meta nodes ][array/object children..]
    const MEAT_NODE_COUNT: usize = size_of::<MetaNode>() / size_of::<Self>();
}

impl<'dom> Value<'dom> {
    /// Create a new `Value` from a null
    #[inline(always)]
    pub const fn new_uinit() -> Self {
        Self {
            typ: NodeMeta(Self::NULL),
            val: NodeValue { uval: 0 },
        }
    }

    /// Create a new `Value` from a i64
    #[inline(always)]
    pub const fn new_i64(val: i64) -> Self {
        Self {
            typ: NodeMeta(Self::SIGNED),
            val: NodeValue { ival: val },
        }
    }

    /// Create a new `Value` from a f64, if not finite return None.
    #[inline(always)]
    pub fn new_f64(val: f64) -> Option<Self> {
        // not support f64::NAN and f64::INFINITY
        if val.is_finite() {
            Some(Self {
                typ: NodeMeta(Self::FLOAT),
                val: NodeValue { fval: val },
            })
        } else {
            None
        }
    }

    /// Create a new `Value` from a f64. Not checking the f64 is finite.
    /// # Safety
    /// The f64 must be finite. Because JSON RFC NOT support `NaN` and `Infinity`.
    #[inline(always)]
    pub unsafe fn new_f64_unchecked(val: f64) -> Self {
        Self {
            typ: NodeMeta(Self::FLOAT),
            val: NodeValue { fval: val },
        }
    }

    /// Create a new `Value` from a u64
    #[inline(always)]
    pub const fn new_u64(val: u64) -> Self {
        Self {
            typ: NodeMeta(Self::UNSIGNED),
            val: NodeValue { uval: val },
        }
    }

    /// Create a new `Value` from a bool
    #[inline(always)]
    pub const fn new_bool(val: bool) -> Self {
        Self {
            typ: NodeMeta(if val { Self::TRUE } else { Self::FALSE }),
            val: NodeValue { uval: 0 },
        }
    }

    /// Create a new `Value` from a empty object
    #[inline(always)]
    pub const fn new_object() -> Self {
        Self {
            typ: NodeMeta(JsonType::Object as u64),
            val: NodeValue { uval: 0 },
        }
    }

    /// Create a new `Value` from a empty array
    #[inline(always)]
    pub const fn new_array() -> Self {
        Self {
            typ: NodeMeta(JsonType::Array as u64),
            val: NodeValue { uval: 0 },
        }
    }

    /// create a new owned string value with the alloctor
    #[inline(always)]
    pub fn new_str(val: &str, alloc: &'dom Bump) -> Self {
        let val = alloc.alloc_str(val);
        Self {
            typ: NodeMeta(Self::STRING | ((val.len() as u64) << Self::LEN_BITS)),
            val: NodeValue {
                str_ptr: val.as_bytes().as_ptr(),
            },
        }
    }

    /// create a new string from static
    #[inline(always)]
    pub fn new_str_static(val: &'static str) -> Self {
        Self {
            typ: NodeMeta(Self::STRING | ((val.len() as u64) << Self::LEN_BITS)),
            val: NodeValue {
                str_ptr: val.as_bytes().as_ptr(),
            },
        }
    }

    /// create a new borrow string, lifetime of Value will limited  by str
    #[inline(always)]
    pub fn new_str_borrow(val: &str) -> Self {
        Self {
            typ: NodeMeta(Self::STRING | ((val.len() as u64) << Self::LEN_BITS)),
            val: NodeValue {
                str_ptr: val.as_bytes().as_ptr(),
            },
        }
    }

    pub fn as_array(&'dom self) -> Option<Array<'dom>> {
        if self.is_array() {
            Some(Array(self))
        } else {
            None
        }
    }

    pub fn as_object(&'dom self) -> Option<Object<'dom>> {
        if self.is_object() {
            Some(Object(self))
        } else {
            None
        }
    }

    fn typ(&self) -> u64 {
        self.typ.0 & 0xff
    }

    fn pointer_mut(&mut self, path: &JsonPointer) -> Option<&mut Self> {
        let mut node = self;
        for p in path {
            if let Some(next) = node.at_pointer_mut(p) {
                node = next;
            } else {
                return None;
            }
        }
        Some(node)
    }

    fn get_mut<I: IndexMut>(&mut self, index: I) -> Option<&mut Self> {
        index.index_into_mut(self)
    }

    fn take(&mut self) -> Self {
        let node = Self {
            val: self.val,
            typ: self.typ,
        };
        self.set_null();
        node
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

    pub(crate) fn get_key(&self, key: &str) -> Option<&Self> {
        debug_assert!(self.is_object());
        if let Some(kv) = self.children::<(Self, Self)>() {
            for (k, v) in kv {
                assert!(k.is_str());
                if k.equal_str(key) {
                    return Some(v);
                }
            }
        }
        None
    }

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

    pub(crate) fn get_key_mut(&mut self, key: &str) -> Option<&mut Self> {
        if let Some(kv) = self.children_mut::<(Self, Self)>() {
            for (k, v) in kv.iter_mut() {
                debug_assert!(k.is_str());
                if k.equal_str(key) {
                    return Some(v);
                }
            }
        }
        None
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
            PointerNode::Key(key) => self.get_key_mut(key),
            PointerNode::Index(index) => self.get_index_mut(*index),
        }
    }

    fn reserve_array(&mut self, additional: usize, alloc: &'dom Bump) {
        debug_assert!(self.is_array());
        let new_cap = self.len() + additional;
        if new_cap > self.capacity() {
            let children = alloc.alloc_slice_fill_default(new_cap + Self::MEAT_NODE_COUNT);
            if self.capacity() == 0 {
                let first = MetaNode::from_nodes(children);
                first.cap = new_cap as u64;
                self.set_meta_ptr(first);
                return;
            }
            let old = self.children::<Self>().unwrap();
            unsafe {
                let src = old.as_ptr();
                let dst =
                    children[Self::MEAT_NODE_COUNT..old.len() + Self::MEAT_NODE_COUNT].as_mut_ptr();
                let count = self.len();
                std::ptr::copy_nonoverlapping(src, dst, count);
            }
            let first = MetaNode::from_nodes(children);
            first.cap = new_cap as u64;
            self.set_meta_ptr(first);
        }
    }

    fn reserve_object(&mut self, additional: usize, alloc: &'dom Bump) {
        debug_assert!(self.is_object());
        let new_cap = self.len() + additional;
        if new_cap > self.capacity() {
            let children: &mut [Value] =
                alloc.alloc_slice_fill_default(new_cap * 2 + Self::MEAT_NODE_COUNT);
            if self.capacity() == 0 {
                let first = MetaNode::from_nodes(children);
                first.cap = new_cap as u64;
                self.set_meta_ptr(first);
                return;
            }
            let old = self.children::<(Self, Self)>().unwrap();
            let len = old.len() * 2;
            let old = unsafe { from_raw_parts(old.as_ptr() as *const Value, len) };
            unsafe {
                let src = old.as_ptr();
                let dst =
                    children[Self::MEAT_NODE_COUNT..old.len() + Self::MEAT_NODE_COUNT].as_mut_ptr();
                std::ptr::copy_nonoverlapping(src, dst, len);
            }
            let first = MetaNode::from_nodes(children);
            first.cap = new_cap as u64;
            self.set_meta_ptr(first);
        }
    }

    fn set_meta_ptr(&mut self, first: &mut MetaNode) {
        self.val.first = unsafe { NonNull::new_unchecked(first) };
    }

    fn equal_str(&self, val: &str) -> bool {
        debug_assert!(self.is_str());
        self.str().len() == val.len() && self.str() == val
    }

    pub(crate) fn len(&self) -> usize {
        debug_assert!(self.is_object() || self.is_str() || self.is_array());
        self.typ.0 as usize >> 8
    }

    fn has_children(&self) -> bool {
        unsafe { self.val.uval != 0 }
    }

    fn capacity(&self) -> usize {
        debug_assert!(self.is_object() || self.is_array());
        if self.has_children() {
            let first = unsafe { self.val.first.as_ref() };
            first.cap as usize
        } else {
            0
        }
    }

    fn append_array(&mut self, node: Self, alloc: &'dom Bump) {
        self.reserve_array(1, alloc);
        debug_assert!(self.capacity() > self.len());
        let children = self.children_mut_unchecked::<Self>();
        let len = children.len();
        unsafe {
            *children.as_mut_ptr().add(len) = node;
        }
        self.add_len(1);
    }

    pub(crate) fn append_object(&mut self, pair: (Self, Self), alloc: &'dom Bump) -> &mut Self {
        self.reserve_object(1, alloc);
        let children = self.children_mut_unchecked::<(Self, Self)>();
        let len = children.len();
        let ret = unsafe {
            let ptr = children.as_mut_ptr().add(len);
            *ptr = pair;
            &mut (*ptr).1
        };
        self.add_len(1);
        ret
    }

    fn remove_in_object(&mut self, k: &str) -> Option<Self> {
        debug_assert!(self.is_object());
        if let Some(i) = self.get_key_offset(k) {
            let children = self.children_mut_unchecked::<(Self, Self)>();
            let node = (children[i].0.take(), children[i].1.take());
            // move the later nodes to first
            let len: usize = children.len();
            unsafe {
                let dst = children.as_mut_ptr().add(i);
                let src = children.as_ptr().add(i + 1);

                let size = (len - i - 1) * size_of::<Self>();
                std::ptr::copy(src, dst, size);
            }
            self.add_len(-1);
            Some(node.1)
        } else {
            None
        }
    }

    // if object, return as a doubled size slice
    pub(crate) fn children<T>(&self) -> Option<&[T]> {
        if self.has_children() {
            Some(self.children_unchecked::<T>())
        } else {
            None
        }
    }

    fn children_unchecked<T>(&self) -> &[T] {
        unsafe {
            let start = self.val.nptr.as_ptr().add(Self::MEAT_NODE_COUNT);
            let ptr = start as *const T;
            let len = self.len();
            from_raw_parts(ptr, len)
        }
    }

    fn children_mut<T>(&mut self) -> Option<&mut [T]> {
        if self.has_children() {
            Some(self.children_mut_unchecked::<T>())
        } else {
            None
        }
    }

    fn children_mut_unchecked<T>(&mut self) -> &mut [T] {
        unsafe {
            let start = self.val.nptr.as_ptr().add(Self::MEAT_NODE_COUNT);
            let ptr = start as *mut T;
            let len = self.len();
            from_raw_parts_mut(ptr, len)
        }
    }

    fn str(&self) -> &str {
        debug_assert!(self.typ() == Self::STRING);
        let s = unsafe {
            let ptr = self.val.own_ptr;
            let len = self.len();
            let slice = std::slice::from_raw_parts(ptr, len);
            std::str::from_utf8_unchecked(slice)
        };
        s
    }

    fn array(&self) -> &[Self] {
        if self.len() == 0 {
            return &[];
        }
        let slice = unsafe {
            let ptr = self.val.nptr;
            let len = self.len();
            // add 1 to skip the metanode
            std::slice::from_raw_parts(ptr.as_ptr().add(1), len)
        };
        slice
    }

    fn object(&self) -> &[(Self, Self)] {
        if self.len() == 0 {
            return &[];
        }
        let slice = unsafe {
            let ptr = self.val.nptr.as_ptr().add(1);
            let len = self.len();
            // add 1 to skip the metanode
            std::slice::from_raw_parts(ptr as *const (Self, Self), len)
        };
        slice
    }

    fn set_null(&mut self) {
        self.typ.0 = Self::NULL;
    }

    fn set_len(&mut self, len: usize) {
        debug_assert!(self.len() == 0);
        self.typ.0 |= (len as u64) << Self::LEN_BITS;
    }

    fn add_len(&mut self, inc: isize) {
        if inc > 0 {
            self.typ.0 += (inc as u64) << Self::LEN_BITS;
        } else {
            self.typ.0 -= ((-inc) as u64) << Self::LEN_BITS;
        }
    }

    fn set_bool(&mut self, val: bool) {
        if val {
            self.typ.0 = Self::TRUE;
        } else {
            self.typ.0 = Self::FALSE;
        }
    }

    fn i64(&self) -> i64 {
        unsafe { self.val.ival }
    }

    fn u64(&self) -> u64 {
        unsafe { self.val.uval }
    }

    fn f64(&self) -> f64 {
        unsafe { self.val.fval }
    }
}

struct MetaNode {
    cap: u64,
    _remain: u64,
}

impl MetaNode {
    fn from_nodes<'dom>(slice: &'dom mut [Value]) -> &'dom mut Self {
        debug_assert!(slice.len() >= Value::MEAT_NODE_COUNT);
        unsafe { &mut *(slice.as_mut_ptr() as *mut MetaNode) }
    }
}

struct ValueInner {
    _typ: u64,
    _val: u64,
}

pub struct Document {
    root: ValueInner,
    alloc: Bump,
}

unsafe impl Send for Document {}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_value().fmt(f)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a json into a document.
pub fn dom_from_str(json: &str) -> Result<Document> {
    let mut dom = Document::new();
    dom.parse_bytes_impl(json.as_bytes())?;
    Ok(dom)
}

/// Parse a json into a document.
///
/// If the json is valid utf-8, recommend to use `dom_from_slice_unchecked` instead.
pub fn dom_from_slice(json: &[u8]) -> Result<Document> {
    // validate the utf-8 at first for slice
    let json = {
        let json = from_utf8(json)?;
        json.as_bytes()
    };

    let mut dom = Document::new();
    dom.parse_bytes_impl(json)?;
    Ok(dom)
}

/// Parse a json into a document.
///
/// # Safety
/// The json must be valid utf-8.
pub unsafe fn dom_from_slice_unchecked(json: &[u8]) -> Result<Document> {
    let mut dom = Document::new();
    dom.parse_bytes_impl(json)?;
    Ok(dom)
}

/// ValueMut is a mutable reference to a `Value`.
#[derive(Debug)]
pub struct ValueMut<'dom> {
    val: &'dom mut Value<'dom>,
    alloc: &'dom Bump,
}

impl<'d> JsonValue for ValueMut<'d> {
    type ValueType<'dom> = &'dom Value<'dom> where Self: 'dom;

    fn as_bool(&self) -> Option<bool> {
        self.val.as_bool()
    }

    fn as_number(&self) -> Option<Number> {
        self.val.as_number()
    }

    fn get_type(&self) -> JsonType {
        self.val.get_type()
    }

    fn as_str(&self) -> Option<&str> {
        self.val.as_str()
    }

    fn get<I: Index>(&'_ self, index: I) -> Option<Self::ValueType<'_>> {
        index.value_index_into(self.val)
    }

    fn pointer(&'_ self, path: &JsonPointer) -> Option<Self::ValueType<'_>> {
        self.val.pointer(path)
    }
}

impl<'dom> ValueMut<'dom> {
    pub fn allocator(&self) -> &'dom Bump {
        self.alloc
    }

    pub fn into_object_mut(self) -> Option<ObjectMut<'dom>> {
        if self.is_object() {
            Some(ObjectMut(self))
        } else {
            None
        }
    }

    pub fn into_array_mut(self) -> Option<ArrayMut<'dom>> {
        if self.is_array() {
            Some(ArrayMut(self))
        } else {
            None
        }
    }

    pub fn pointer_mut(&'dom mut self, path: &JsonPointer) -> Option<ValueMut<'dom>> {
        if let Some(val) = self.val.pointer_mut(path) {
            Some(Self {
                val,
                alloc: self.alloc,
            })
        } else {
            None
        }
    }

    pub fn get_mut<I: IndexMut>(&'dom mut self, index: I) -> Option<ValueMut<'dom>> {
        if let Some(val) = index.index_into_mut(self.val) {
            Some(Self {
                val,
                alloc: self.alloc,
            })
        } else {
            None
        }
    }

    pub fn take(&mut self) -> Value<'dom> {
        self.val.take()
    }
}

impl Document {
    const PADDING_SIZE: usize = 64;

    pub fn new() -> Document {
        Self {
            alloc: Bump::new(),
            root: ValueInner { _typ: 0, _val: 0 },
        }
    }

    pub fn as_value(&self) -> &Value {
        unsafe { transmute(&self.root) }
    }

    pub fn as_value_mut(&'_ mut self) -> ValueMut<'_> {
        ValueMut {
            val: unsafe { transmute(&mut self.root) },
            alloc: &self.alloc,
        }
    }

    pub fn as_array_mut(&'_ mut self) -> Option<ArrayMut<'_>> {
        if self.as_value().is_array() {
            Some(ArrayMut(ValueMut {
                val: unsafe { transmute(&mut self.root) },
                alloc: &self.alloc,
            }))
        } else {
            None
        }
    }

    pub fn as_object_mut(&'_ mut self) -> Option<ObjectMut<'_>> {
        if self.as_value().is_object() {
            Some(ObjectMut(ValueMut {
                val: unsafe { transmute(&mut self.root) },
                alloc: &self.alloc,
            }))
        } else {
            None
        }
    }

    fn parse_bytes_impl(&mut self, json: &[u8]) -> Result<()> {
        let alloc = &self.alloc;
        let len = json.len();

        // allocate the padding buffer for the input json
        let real_size = len + Self::PADDING_SIZE;
        let layout = Layout::array::<u8>(real_size).map_err(Error::custom)?;
        let dst = alloc.try_alloc_layout(layout).map_err(Error::custom)?;
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
        let slice = UncheckedSliceRead::new(json_buf);
        let mut parser = Parser::new(slice);

        // a simple wrapper for visitor
        #[derive(Debug)]
        struct DocumentVisitor<'a> {
            alloc: &'a Bump,
            nodes: Vec<Value<'static>>,
            parent: usize,
        }

        impl<'a> DocumentVisitor<'a> {
            // the array and object's logic is same.
            fn visit_container(&mut self, len: usize) -> bool {
                let visitor = self;
                let alloc = visitor.alloc;
                let parent = visitor.parent;
                let old = unsafe { visitor.nodes[parent].val.parent as usize };
                visitor.parent = old;
                if len > 0 {
                    unsafe {
                        let visited_children = &visitor.nodes[(parent + 1)..];
                        let real_count = visited_children.len() + Value::MEAT_NODE_COUNT;
                        let layout = {
                            if let Ok(layout) = Layout::array::<Value>(real_count) {
                                layout
                            } else {
                                return false;
                            }
                        };
                        let mut children = {
                            if let Ok(c) = alloc.try_alloc_layout(layout) {
                                c
                            } else {
                                return false;
                            }
                        };

                        // copy visited nodes into document
                        let src = visited_children.as_ptr();
                        let dst = children.as_ptr() as *mut Value;
                        let dst = dst.add(Value::MEAT_NODE_COUNT);
                        std::ptr::copy_nonoverlapping(src, dst, visited_children.len());

                        // set the capacity and length
                        let meta = &mut *(children.as_mut() as *mut _ as *mut MetaNode);
                        meta.cap = len as u64;
                        let container = &mut visitor.nodes[parent];
                        container.set_len(len);
                        container.val.nptr =
                            NonNull::new_unchecked(children.as_mut() as *mut _ as *mut Value);

                        // must reset the length, because we copy the children into bumps
                        visitor.nodes.set_len(parent + 1);
                    }
                }
                true
            }

            fn push_node(&mut self, node: Value<'static>) -> bool {
                if self.nodes.len() == self.nodes.capacity() {
                    false
                } else {
                    self.nodes.push(node);
                    true
                }
            }
        }

        impl<'de, 'a: 'de> JsonVisitor<'de> for DocumentVisitor<'a> {
            #[inline(always)]
            fn visit_bool(&mut self, val: bool) -> bool {
                self.push_node(Value::new_bool(val))
            }

            #[inline(always)]
            fn visit_f64(&mut self, val: f64) -> bool {
                // # Safety
                // we have checked the f64 in parsing number.
                let node = unsafe { Value::new_f64_unchecked(val) };
                self.push_node(node)
            }

            #[inline(always)]
            fn visit_i64(&mut self, val: i64) -> bool {
                self.push_node(Value::new_i64(val))
            }

            #[inline(always)]
            fn visit_u64(&mut self, val: u64) -> bool {
                self.push_node(Value::new_u64(val))
            }

            #[inline(always)]
            fn visit_array_start(&mut self, _hint: usize) -> bool {
                let ret = self.push_node(Value::new_array());
                // record the parent container position
                let len = self.nodes.len();
                self.nodes[len - 1].val.parent = self.parent as u64;
                self.parent = len - 1;
                ret
            }

            #[inline(always)]
            fn visit_array_end(&mut self, len: usize) -> bool {
                self.visit_container(len)
            }

            #[inline(always)]
            fn visit_object_start(&mut self, _hint: usize) -> bool {
                let ret = self.push_node(Value::new_object());
                let len = self.nodes.len();
                self.nodes[len - 1].val.parent = self.parent as u64;
                self.parent = len - 1;
                ret
            }

            #[inline(always)]
            fn visit_object_end(&mut self, len: usize) -> bool {
                self.visit_container(len)
            }

            #[inline(always)]
            fn visit_null(&mut self) -> bool {
                self.push_node(Value::new_uinit())
            }

            #[inline(always)]
            fn visit_str(&mut self, value: &str) -> bool {
                let alloc = self.alloc;
                let value = alloc.alloc_str(value);
                self.push_node(Value::new_str_borrow(value))
            }

            #[inline(always)]
            fn visit_borrowed_str(&mut self, value: &'de str) -> bool {
                self.push_node(Value::new_str_borrow(value))
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

        let alloc = &self.alloc;
        // optimize: use a pre-allocated vec.
        // If json is valid, the max number of value nodes should be
        // half of the valid json length + 2. like as [1,2,3,1,2,3...]
        // if the capacity is not enough, we will return a error.
        let nodes = Vec::with_capacity((json.len() / 2) + 2);
        let parent = 0;
        let mut visitor = DocumentVisitor {
            alloc,
            nodes,
            parent,
        };
        parser.parse_value_goto(&mut visitor)?;
        // check trailing spaces
        parser.parse_trailing()?;
        self.root = unsafe { transmute(visitor.nodes[0].take()) };
        Ok(())
    }
}

impl JsonValue for Document {
    type ValueType<'dom> = &'dom Value<'dom> where Self: 'dom;

    fn get_type(&self) -> JsonType {
        self.as_value().get_type()
    }

    fn as_bool(&self) -> Option<bool> {
        self.as_value().as_bool()
    }

    fn as_number(&self) -> Option<Number> {
        self.as_value().as_number()
    }

    fn as_str(&self) -> Option<&str> {
        self.as_value().as_str()
    }

    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        self.as_value().get(index)
    }

    fn pointer(&self, path: &JsonPointer) -> Option<Self::ValueType<'_>> {
        self.as_value().pointer(path)
    }
}

impl Serialize for Document {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.as_value().serialize(serializer)
    }
}
impl<'dom> Serialize for Value<'dom> {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        match self.typ() {
            Self::NULL => serializer.serialize_unit(),
            Self::TRUE => serializer.serialize_bool(true),
            Self::FALSE => serializer.serialize_bool(false),
            Self::SIGNED => serializer.serialize_i64(self.i64()),
            Self::UNSIGNED => serializer.serialize_u64(self.u64()),
            Self::FLOAT => serializer.serialize_f64(self.f64()),
            Self::STRING => serializer.serialize_str(self.str()),
            Self::ARRAY => {
                let nodes = self.array();
                let mut seq = tri!(serializer.serialize_seq(Some(nodes.len())));
                for n in nodes {
                    tri!(seq.serialize_element(n));
                }
                seq.end()
            }
            Self::OBJECT => {
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
    use crate::{
        error::{make_error, Result},
        pointer,
    };
    use std::{collections::HashMap, path::Path};

    fn test_value(data: &str) -> Result<()> {
        let serde_value: serde_json::Result<serde_json::Value> = serde_json::from_str(data);
        let dom = dom_from_slice(data.as_bytes());
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
        let dom = dom_from_slice(data.as_bytes()).unwrap();
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
    fn test_node_from_files() {
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
            let ret = dom_from_slice(data.as_bytes());
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
        let dom = dom_from_str(TEST_JSON).unwrap();
        let value = dom.as_value();
        assert!(dom.get("bool").is_true());
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
        let dom = dom_from_str(TEST_JSON).unwrap();
        let value = dom.as_value();
        assert_eq!(dom.get("int").as_i64().unwrap(), -1);
        assert_eq!(value.get("int").as_i64().unwrap(), -1);
        assert_eq!(value["array"].get(0).as_i64().unwrap(), 1);

        assert_eq!(dom.pointer(&pointer!["array", 2]).as_i64().unwrap(), 3);
        assert_eq!(value.pointer(&pointer!["array", 2]).as_u64().unwrap(), 3);

        assert_eq!(
            dom.pointer(&pointer!["object", "a"]).as_str().unwrap(),
            "aaa"
        );
        assert_eq!(
            value.pointer(&pointer!["object", "a"]).as_str().unwrap(),
            "aaa"
        );
        assert_eq!(dom.pointer(&pointer!["objempty", "a"]).as_str(), None);
        assert_eq!(value.pointer(&pointer!["objempty", "a"]).as_str(), None);

        assert_eq!(dom.pointer(&pointer!["arrempty", 1]).as_str(), None);
        assert_eq!(value.pointer(&pointer!["arrempty", 1]).as_str(), None);

        assert!(!dom.pointer(&pointer!["unknown"]).is_str());
        assert!(!value.pointer(&pointer!["unknown"]).is_str());
    }

    #[test]
    fn test_value_object() {
        let mut dom = dom_from_str(TEST_JSON).unwrap();
        let value = dom.as_value();
        assert!(value.is_object());

        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 10);
        assert!(object.get("bool").as_bool().unwrap());

        let mut object = dom.as_object_mut().unwrap();
        object.insert("inserted", Value::new_bool(true));
        assert_eq!(object.len(), 11);
        assert!(object.contains_key("inserted"));
        assert!(object.remove("inserted").unwrap().is_true());
        assert!(!object.contains_key("inserted"));

        object.reserve(12);
        assert_eq!(object.capacity(), 22);

        object.insert("inserted", Value::new_bool(true));
        assert!(object.contains_key("inserted"));
    }

    #[test]
    fn test_value_object_empty() {
        let mut dom = dom_from_str(TEST_JSON).unwrap();
        let value = dom.as_value_mut();
        assert!(value.is_object());

        let mut object = value.into_object_mut().unwrap();
        let mut empty = object
            .get_mut("objempty")
            .and_then(|s| s.into_object_mut())
            .unwrap();

        assert_eq!(empty.len(), 0);
        empty.insert(
            "inserted",
            Value::new_str("new inserted", empty.allocator()),
        );
        empty.insert("inserted2", Value::new_bool(true));
        assert_eq!(empty.len(), 2);
        assert!(empty.remove("inserted2").is_true());
        assert!(empty.contains_key("inserted"));
        let value = empty.get_mut("inserted").unwrap().take();
        assert!(value.as_str().unwrap() == "new inserted");
    }

    #[test]
    fn test_value_array() {
        let mut dom = dom_from_str(TEST_JSON).unwrap();
        let mut root = dom.as_value_mut();
        let value = root.get_mut("array").unwrap();
        assert!(value.is_array());
        let mut array = value.into_array_mut().unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[1].as_u64().unwrap(), 2);
        array.push(Value::new_str_static("pushed"));
        assert!(array[3].is_str());
        array.pop();
        assert!(array[2].is_number());
        assert_eq!(array.len(), 3);

        let iter = array.iter();
        assert_eq!(iter.len(), 3);
        for (i, v) in iter.enumerate() {
            assert_eq!(v.as_u64().unwrap(), (i + 1) as u64);
        }
    }

    #[test]
    fn test_value_array_empty() {
        let mut dom = dom_from_str(TEST_JSON).unwrap();
        let mut root = dom.as_value_mut();
        let mut empty = root
            .get_mut("arrempty")
            .and_then(|s| s.into_array_mut())
            .unwrap();

        assert_eq!(empty.len(), 0);
        empty.push(Value::new_str("new inserted", empty.allocator()));
        empty.push(Value::new_bool(true));
        assert_eq!(empty.len(), 2);
        assert!(empty.pop().is_true());
        let value = empty.get_mut(0).unwrap().take();
        assert!(value.as_str().unwrap() == "new inserted");
    }

    #[test]
    fn test_invalid_utf8() {
        let data = [b'"', 0x80, 0x90, b'"'];
        let dom = dom_from_slice(&data);
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 1\n\n\t\"��\"\n\t.^..\n"
        );
        let dom = unsafe { dom_from_slice_unchecked(&data) };
        assert!(dom.is_ok());

        let data = [b'"', b'"', 0x80];
        let dom = dom_from_slice(&data);
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 2\n\n\t\"\"�\n\t..^\n"
        );

        let data = [0x80, b'"', b'"'];
        let dom = dom_from_slice(&data);
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 0\n\n\t�\"\"\n\t^..\n"
        );
    }

    #[test]
    fn test_string_borrow() {
        let s = String::from("borrowed");
        let value2 = Value::new_str_borrow(&s);

        let mut map = HashMap::new();
        map.insert("v2", value2);

        assert_eq!(to_string(&map).unwrap().as_str(), r#"{"v2":"borrowed"}"#);
    }

    #[test]
    fn test_value_from() {
        assert_eq!(Value::from(1_u64).as_u64().unwrap(), 1);
        assert_eq!(Value::from(-1_i64).as_i64().unwrap(), -1);

        assert!(Value::try_from(f64::INFINITY).is_err());
        assert!(Value::try_from(f64::NAN).is_err());
        assert_eq!(
            Value::try_from(f64::MAX).unwrap().as_f64().unwrap(),
            f64::MAX
        );
    }
}
