use core::mem::size_of;
#[cfg(feature = "sort_keys")]
use std::collections::BTreeMap;
use std::{
    alloc::Layout,
    fmt::{Debug, Display, Formatter},
    mem::{transmute, ManuallyDrop},
    ptr::NonNull,
    slice::from_raw_parts,
    str::from_utf8_unchecked,
    sync::Arc,
};

#[cfg(not(feature = "sort_keys"))]
use ahash::AHashMap;
use faststr::FastStr;
use ref_cast::RefCast;
use serde::ser::{Serialize, SerializeMap, SerializeSeq};

use super::{
    object::Pair,
    shared::Shared,
    tls_buffer::NodeBuf,
    value_trait::{JsonContainerTrait, JsonValueMutTrait},
    visitor::JsonVisitor,
};
use crate::{
    config::DeserializeCfg,
    error::Result,
    index::Index,
    parser::Parser,
    reader::{PaddedSliceRead, Reader},
    serde::tri,
    util::string::str_from_raw_parts,
    value::{array::Array, object::Object, value_trait::JsonValueTrait},
    JsonNumberTrait, JsonType, Number, RawNumber,
};

/// Inline memcpy for Value-sized (16-byte) chunks using AVX2 SIMD.
/// Only compiled when AVX2 is available; non-AVX2 uses copy_nonoverlapping
/// directly to preserve embedded pointer provenance (Miri compatibility).
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
pub(super) unsafe fn inline_copy_values(
    src: *const ManuallyDrop<Value>,
    dst: *mut ManuallyDrop<Value>,
    count: usize,
) {
    use core::arch::x86_64::*;
    let mut s = src as *const u8;
    let mut d = dst as *mut u8;
    let blocks = count / 8;
    for _ in 0..blocks {
        let v0 = _mm256_loadu_si256(s as *const __m256i);
        let v1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
        let v2 = _mm256_loadu_si256(s.add(64) as *const __m256i);
        let v3 = _mm256_loadu_si256(s.add(96) as *const __m256i);
        _mm256_storeu_si256(d as *mut __m256i, v0);
        _mm256_storeu_si256(d.add(32) as *mut __m256i, v1);
        _mm256_storeu_si256(d.add(64) as *mut __m256i, v2);
        _mm256_storeu_si256(d.add(96) as *mut __m256i, v3);
        s = s.add(128);
        d = d.add(128);
    }
    let rem = count & 7;
    match rem >> 1 {
        3 => {
            _mm256_storeu_si256(d as *mut __m256i, _mm256_loadu_si256(s as *const __m256i));
            _mm256_storeu_si256(
                d.add(32) as *mut __m256i,
                _mm256_loadu_si256(s.add(32) as *const __m256i),
            );
            _mm256_storeu_si256(
                d.add(64) as *mut __m256i,
                _mm256_loadu_si256(s.add(64) as *const __m256i),
            );
            s = s.add(96);
            d = d.add(96);
        }
        2 => {
            _mm256_storeu_si256(d as *mut __m256i, _mm256_loadu_si256(s as *const __m256i));
            _mm256_storeu_si256(
                d.add(32) as *mut __m256i,
                _mm256_loadu_si256(s.add(32) as *const __m256i),
            );
            s = s.add(64);
            d = d.add(64);
        }
        1 => {
            _mm256_storeu_si256(d as *mut __m256i, _mm256_loadu_si256(s as *const __m256i));
            s = s.add(32);
            d = d.add(32);
        }
        _ => {}
    }
    if rem & 1 != 0 {
        _mm_storeu_si128(d as *mut __m128i, _mm_loadu_si128(s as *const __m128i));
    }
}

/// Represents any valid JSON value.
///
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
/// # Notes
///
/// Not use any unsafe invalid_reference_casting for `Value`, it will cause UB.
///
/// ```rust,no_run
/// use sonic_rs::{from_str, Value};
/// let json = r#"["a", "b", "c"]"#;
/// let root: Value = from_str(json).unwrap();
/// let immref = &root["b"];
///
/// // This is dangerous, will coredump when using sanitizer
/// #[allow(invalid_reference_casting)]
/// let ub_cast = unsafe { &mut *(immref as *const _ as *mut Value) };
/// let _ub = std::mem::take(ub_cast);
/// ```
#[repr(C)]
pub struct Value {
    pub(crate) meta: Meta,
    pub(crate) data: Data,
}

#[rustfmt::skip]
// A compact and mutable JSON Value.
//
//  Thera are three kind nodes into the Value:
//  - Static Node: no need drop
//  - Owned Node : mutable
//  - Shared Node: in SharedDom, not mutable
//
// |  Kind        | 3 bits | 5 bits |       24 bits     |     ---->  32 bits ---->       |    32 bits    |    32 bits    |       limit          |
// |--------------|-----------------|-------------------|--------------------------------|-------------------------------|----------------------|
// |   Null       |   0    |   0    |                                                    +                               |                      |
// |   True       |   0    |   1    |                                                    +                               |                      |
// |   False      |   0    |   2    |                                                    +                               |                      |
// |   I64        |   0    |   3    |                                                    +             i64               |                      |
// |   U64        |   0    |   4    |                                                    +             u64               |                      |
// |   F64        |   0    |   5    |                                                    +             f64               |                      |
// |  empty arr   |   0    |   6    |                                                                                    |
// |  empty obj   |   0    |   7    |                                                                                    |
// |  static str  |   0    |   8    |                   |           string length        +          *const u8            | excced will fallback |
// |   faststr    |   1    |   0    |                                                    +         Box<FastStr>          |                      |
// |rawnum_faststr|   1    |   1    |                                                    +         Box<FastStr>          |                      |
// |   arr_mut    |   1    |   2    |                                                    +        Arc<Vec<Node>>         |                      |
// |   obj_mut    |   1    |   3    |                                                    + Arc<AHashMap<FastStr, Value>> |                      |
// |   str_node   |   2    |        node idx            |           string length        +          *const u8            |    max len 2^32      |
// | raw_num_node |   3    |        node idx            |           string length        +          *const u8            |    max len 2^32      |
// |   arr_node   |   4    |        node idx            |           array length         +          *const Node          |    max len 2^32      |
// |   obj_node   |   5    |        node idx            |           object length        +          *const Pair          |    max len 2^32      |
// |   _reserved  |   6    |
// |  root_node   |   7    |      *const ShardDom (from Arc, MUST aligned 8)             +      *const Node (head)       |                      |
//
// NB: we will check the JSON length when parsing, if JSON is >= 4GB, will return a error, so we will not check the limits when parsing or using dom.
#[allow(clippy::box_collection)]
#[repr(C)]
pub(crate) union Data {
    pub(crate) uval: u64,
    pub(crate) ival: i64,
    pub(crate) fval: f64,
    pub(crate) static_str: NonNull<u8>,

    pub(crate) dom_str: NonNull<u8>,
    pub(crate) arr_elems: NonNull<Value>,
    pub(crate) obj_pairs: NonNull<Pair>,

    pub(crate) root: NonNull<Value>,

    pub(crate) str_own: ManuallyDrop<Box<FastStr>>,
    #[cfg(not(feature = "sort_keys"))]
    pub(crate) obj_own: ManuallyDrop<Arc<AHashMap<FastStr, Value>>>,
    #[cfg(feature="sort_keys")]
    pub(crate) obj_own: ManuallyDrop<Arc<BTreeMap<FastStr, Value>>>,
    pub(crate) arr_own: ManuallyDrop<Arc<Vec<Value>>>,

    pub(crate) parent: u64,
}

/// Compact metadata for a `Value` node.
///
/// On 64-bit targets this is a union of `u64` and `*const Shared`.
/// Using a union preserves pointer provenance through storage, enabling
/// strict-provenance-compatible round-trips for the root_node variant
/// without needing `expose_provenance` / `with_exposed_provenance`.
///
/// On 32-bit targets (e.g. wasm32) the pointer is only 4 bytes while `val`
/// is 8 bytes, so writing through a `ptr` field would leave the upper half
/// uninitialized — reading back via `val` would be UB.  Therefore on 32-bit
/// we fall back to a plain `u64` struct with exposed provenance for the
/// root_node round-trip.
///
/// All non-root variants always read/write through the `val` field.
/// The root variant writes/reads through the `ptr` field (64-bit) or
/// through `val` with expose/recover (32-bit).
#[derive(Copy, Clone)]
#[cfg(target_pointer_width = "64")]
#[repr(C)]
pub(crate) union Meta {
    val: u64,
    ptr: *const Shared,
}

#[derive(Copy, Clone)]
#[cfg(not(target_pointer_width = "64"))]
#[repr(transparent)]
pub(crate) struct Meta {
    val: u64,
}

// Safety: Meta contains either a plain integer or a pointer derived from
// Arc<Shared> (which is Send+Sync). The pointer is never dereferenced
// through Meta directly — it is only unpacked and used behind Arc's
// reference counting.
#[cfg(target_pointer_width = "64")]
unsafe impl Send for Meta {}
#[cfg(target_pointer_width = "64")]
unsafe impl Sync for Meta {}

impl Meta {
    const STAIC_NODE: u64 = 0;
    const NULL: u64 = (0 << Self::KIND_BITS);
    const TRUE: u64 = (1 << Self::KIND_BITS);
    const FALSE: u64 = (2 << Self::KIND_BITS);
    const I64: u64 = (3 << Self::KIND_BITS);
    const U64: u64 = (4 << Self::KIND_BITS);
    const F64: u64 = (5 << Self::KIND_BITS);
    const EMPTY_ARR: u64 = (6 << Self::KIND_BITS);
    const EMPTY_OBJ: u64 = (7 << Self::KIND_BITS);
    const STATIC_STR: u64 = (8 << Self::KIND_BITS);

    const OWNED_NODE: u64 = 1;
    const FASTSTR: u64 = 1 | (0 << Self::KIND_BITS);
    const RAWNUM_FASTSTR: u64 = 1 | (1 << Self::KIND_BITS);
    const ARR_MUT: u64 = 1 | (2 << Self::KIND_BITS);
    const OBJ_MUT: u64 = 1 | (3 << Self::KIND_BITS);

    const STR_NODE: u64 = 2;
    const RAWNUM_NODE: u64 = 3;
    const ARR_NODE: u64 = 4;
    const OBJ_NODE: u64 = 5;

    const ROOT_NODE: u64 = 7;

    const KIND_BITS: u64 = 3;
    const KIND_MASK: u64 = (1 << Self::KIND_BITS) - 1;

    const TYPE_BITS: u64 = 8;
    const TYPE_MASK: u64 = (1 << Self::TYPE_BITS) - 1;

    const IDX_MASK: u64 = ((1 << Self::LEN_OFFSET) - 1) & !Self::KIND_MASK;
    const LEN_OFFSET: u64 = 32;
}

impl Meta {
    pub const fn new(typ: u64) -> Self {
        Self { val: typ }
    }

    fn pack_dom_node(kind: u64, idx: u32, len: u32) -> Self {
        debug_assert!(matches!(
            kind,
            Self::ARR_NODE | Self::OBJ_NODE | Self::STR_NODE | Self::RAWNUM_NODE
        ));
        let idx = idx as u64;
        let len = len as u64;
        let val = kind | (idx << Self::KIND_BITS) | (len << Self::LEN_OFFSET);
        Self { val }
    }

    fn pack_static_str(kind: u64, len: usize) -> Self {
        assert!(len < (u32::MAX as usize));
        assert!(kind == Self::STATIC_STR);
        let val = kind | ((len as u64) << Self::LEN_OFFSET);
        Self { val }
    }

    /// Pack a `*const Shared` pointer into a root Meta node.
    ///
    /// On 64-bit: stores the tagged pointer through the union `ptr` field,
    /// preserving provenance (strict-provenance compatible).
    ///
    /// On 32-bit: stores through `val` as a u64, using exposed provenance
    /// for the Arc::increment_strong_count call.  The pointer address fits
    /// in the low 32 bits of the u64.
    #[cfg(target_pointer_width = "64")]
    fn pack_shared(ptr: *const Shared) -> Self {
        // Arc::increment_strong_count needs provenance covering the full
        // Arc allocation (header + data). The caller's ptr may come from
        // &Shared (narrow provenance), so the Arc allocation must be exposed
        // beforehand (done in parse_with_padding / Deserializer).
        let addr = ptr.expose_provenance();
        let wide_ptr = std::ptr::with_exposed_provenance::<Shared>(addr);
        unsafe { Arc::increment_strong_count(wide_ptr) };
        // Store tagged pointer through the union ptr field — provenance preserved.
        let tagged = ptr.map_addr(|a| a | Self::ROOT_NODE as usize);
        Self { ptr: tagged }
    }

    #[cfg(not(target_pointer_width = "64"))]
    fn pack_shared(ptr: *const Shared) -> Self {
        let addr = ptr.expose_provenance();
        let wide_ptr = std::ptr::with_exposed_provenance::<Shared>(addr);
        unsafe { Arc::increment_strong_count(wide_ptr) };
        let val = addr as u64 | Self::ROOT_NODE;
        Self { val }
    }

    /// Read the integer representation of this Meta.
    #[inline(always)]
    #[cfg(target_pointer_width = "64")]
    fn read_val(&self) -> u64 {
        // Safety: reading `val` when `ptr` was written is sound on 64-bit —
        // both fields are 8 bytes and we only inspect integer bits.
        unsafe { self.val }
    }

    #[inline(always)]
    #[cfg(not(target_pointer_width = "64"))]
    fn read_val(&self) -> u64 {
        self.val
    }

    fn get_kind(&self) -> u64 {
        self.read_val() & Self::KIND_MASK
    }

    fn get_type(&self) -> u64 {
        let val = self.read_val();
        let typ = val & Self::TYPE_MASK;
        let kind = val & Self::KIND_MASK;
        match kind {
            Self::STAIC_NODE | Self::OWNED_NODE => typ,
            Self::STR_NODE | Self::RAWNUM_NODE | Self::ARR_NODE | Self::OBJ_NODE => {
                typ & Self::KIND_MASK
            }
            Self::ROOT_NODE => typ & Self::KIND_MASK,
            _ => unreachable!("unknown kind {kind}"),
        }
    }

    fn unpack_dom_node(&self) -> NodeMeta {
        debug_assert!(self.in_shared());
        let val = self.read_val();
        let idx = (val & Self::IDX_MASK) >> Self::KIND_BITS;
        let len = val >> Self::LEN_OFFSET;
        NodeMeta {
            idx: idx as u32,
            len: len as u32,
        }
    }

    /// Recover the `*const Shared` from a root Meta node.
    ///
    /// On 64-bit: reads through the union `ptr` field (provenance preserved).
    /// On 32-bit: recovers via `with_exposed_provenance`.
    #[cfg(target_pointer_width = "64")]
    fn unpack_root(&self) -> *const Shared {
        debug_assert!(self.get_kind() == Self::ROOT_NODE);
        unsafe { self.ptr.map_addr(|a| a & !(Self::ROOT_NODE as usize)) }
    }

    #[cfg(not(target_pointer_width = "64"))]
    fn unpack_root(&self) -> *const Shared {
        debug_assert!(self.get_kind() == Self::ROOT_NODE);
        let addr = (self.val & !Self::ROOT_NODE) as usize;
        std::ptr::with_exposed_provenance::<Shared>(addr)
    }

    fn has_strlen(&self) -> bool {
        matches!(
            self.get_type(),
            Self::STR_NODE | Self::RAWNUM_NODE | Self::STATIC_STR
        )
    }

    fn in_shared(&self) -> bool {
        matches!(
            self.get_type(),
            Self::STR_NODE | Self::RAWNUM_NODE | Self::ARR_NODE | Self::OBJ_NODE
        )
    }

    fn unpack_strlen(&self) -> usize {
        debug_assert!(self.has_strlen());
        (self.read_val() >> Self::LEN_OFFSET) as usize
    }
}

struct NodeMeta {
    idx: u32,
    len: u32,
}

struct NodeInDom<'a> {
    node: &'a Value,
    dom: &'a Shared,
}

impl<'a> NodeInDom<'a> {
    #[inline(always)]
    fn get_inner(&self) -> ValueRefInner<'a> {
        let typ = self.node.meta.get_type();
        match typ {
            Meta::STR_NODE => ValueRefInner::Str(self.unpack_str()),
            Meta::RAWNUM_NODE => ValueRefInner::RawNum(self.unpack_str()),
            Meta::ARR_NODE => ValueRefInner::Array(self.unpack_value_slice()),
            Meta::OBJ_NODE => ValueRefInner::Object(self.unpack_pair_slice()),
            _ => unreachable!("unknown type {typ} in dom"),
        }
    }

    #[inline(always)]
    fn unpack_str(&self) -> &'a str {
        let len = self.node.meta.unpack_dom_node().len as usize;
        let ptr = unsafe { self.node.data.dom_str.as_ptr() };
        unsafe { str_from_raw_parts(ptr, len) }
    }

    #[inline(always)]
    fn unpack_value_slice(&self) -> &'a [Value] {
        let len = self.node.meta.unpack_dom_node().len as usize;
        let elems = unsafe { self.node.data.arr_elems.as_ptr() };
        unsafe { from_raw_parts(elems, len) }
    }

    #[inline(always)]
    fn unpack_pair_slice(&self) -> &'a [Pair] {
        let len = self.node.meta.unpack_dom_node().len as usize;
        let pairs = unsafe { self.node.data.obj_pairs.as_ptr() };
        unsafe { from_raw_parts(pairs, len) }
    }
}

impl<'a> From<NodeInDom<'a>> for Value {
    fn from(value: NodeInDom<'a>) -> Self {
        Self {
            meta: Meta::pack_shared(value.dom as *const _),
            data: Data {
                root: NonNull::from(value.node),
            },
        }
    }
}

/// The value borrowed from the SharedDom
enum ValueDetail<'a> {
    Null,
    Bool(bool),
    Number(Number),
    StaticStr(&'static str),
    FastStr(&'a FastStr),
    RawNumFasStr(&'a FastStr),
    Array(&'a Arc<Vec<Value>>),
    #[cfg(not(feature = "sort_keys"))]
    Object(&'a Arc<AHashMap<FastStr, Value>>),
    #[cfg(feature = "sort_keys")]
    Object(&'a Arc<BTreeMap<FastStr, Value>>),
    Root(NodeInDom<'a>),
    NodeInDom(NodeInDom<'a>),
    EmptyArray,
    EmptyObject,
}

/// ValueRef is a immutable reference helper for `Value`.
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
#[derive(Debug)]
pub enum ValueRef<'a> {
    Null,
    Bool(bool),
    Number(Number),
    String(&'a str),
    Array(&'a Array),
    Object(&'a Object),
}

#[derive(Debug)]
pub enum ValueRefInner<'a> {
    Null,
    Bool(bool),
    Number(Number),
    Str(&'a str),
    RawNum(&'a str),
    Array(&'a [Value]),
    Object(&'a [Pair]),
    #[cfg(not(feature = "sort_keys"))]
    ObjectOwned(&'a Arc<AHashMap<FastStr, Value>>),
    #[cfg(feature = "sort_keys")]
    ObjectOwned(&'a Arc<BTreeMap<FastStr, Value>>),
    EmptyArray,
    EmptyObject,
}

impl<'a> From<&'a [Pair]> for Value {
    fn from(value: &'a [Pair]) -> Self {
        #[cfg(not(feature = "sort_keys"))]
        let mut newd = AHashMap::with_capacity(value.len());
        #[cfg(feature = "sort_keys")]
        let mut newd = BTreeMap::new();

        for (k, v) in value {
            if let Some(k) = k.as_str() {
                newd.insert(FastStr::new(k), v.clone());
            }
        }

        Self {
            meta: Meta::new(Meta::OBJ_MUT),
            data: Data {
                obj_own: ManuallyDrop::new(Arc::new(newd)),
            },
        }
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        if self.meta.get_kind() == Meta::STAIC_NODE || self.meta.in_shared() {
            return;
        }
        // Safety: each arm accesses the Data union field matching the Meta type tag
        match self.meta.get_type() {
            Meta::FASTSTR | Meta::RAWNUM_FASTSTR => unsafe {
                ManuallyDrop::drop(&mut self.data.str_own)
            },
            Meta::ARR_MUT => unsafe { ManuallyDrop::drop(&mut self.data.arr_own) },
            Meta::OBJ_MUT => unsafe { ManuallyDrop::drop(&mut self.data.obj_own) },
            Meta::ROOT_NODE => {
                let dom = self.meta.unpack_root();
                drop(unsafe { Arc::from_raw(dom) });
            }
            _ => unreachable!("should not be dropped"),
        }
    }
}

pub(crate) enum ValueMut<'a> {
    Null,
    Bool,
    Number,
    Str,
    RawNum,
    Array(&'a mut Vec<Value>),
    #[cfg(not(feature = "sort_keys"))]
    Object(&'a mut AHashMap<FastStr, Value>),
    #[cfg(feature = "sort_keys")]
    Object(&'a mut BTreeMap<FastStr, Value>),
}

impl Value {
    fn is_node_kind(&self) -> bool {
        matches!(
            self.meta.get_kind(),
            Meta::ARR_NODE | Meta::OBJ_NODE | Meta::STR_NODE | Meta::RAWNUM_NODE
        )
    }

    pub(crate) fn as_mut(&mut self) -> ValueMut<'_> {
        let typ = self.meta.get_type();
        match typ {
            Meta::NULL => ValueMut::Null,
            Meta::TRUE | Meta::FALSE => ValueMut::Bool,
            Meta::F64 | Meta::I64 | Meta::U64 => ValueMut::Number,
            Meta::STATIC_STR | Meta::STR_NODE | Meta::FASTSTR => ValueMut::Str,
            Meta::RAWNUM_FASTSTR | Meta::RAWNUM_NODE => ValueMut::RawNum,
            Meta::ARR_MUT => ValueMut::Array(unsafe { Arc::make_mut(&mut self.data.arr_own) }),
            Meta::OBJ_MUT => ValueMut::Object(unsafe { Arc::make_mut(&mut self.data.obj_own) }),
            Meta::ROOT_NODE | Meta::EMPTY_ARR | Meta::EMPTY_OBJ => {
                /* convert to mutable */
                self.to_mut();
                self.as_mut()
            }
            _ => unreachable!("should not be access in mutable api"),
        }
    }
    fn to_mut(&mut self) {
        assert!(
            !self.meta.in_shared(),
            "chidlren in shared should not to mut"
        );
        match self.unpack_ref() {
            ValueDetail::Root(indom) => match indom.node.meta.get_type() {
                Meta::ARR_NODE => {
                    let slice = indom.unpack_value_slice();
                    *self = slice.into();
                }
                Meta::OBJ_NODE => {
                    let slice = indom.unpack_pair_slice();
                    *self = slice.into();
                }
                _ => {}
            },
            ValueDetail::EmptyArray => *self = Value::new_array_with(8),
            ValueDetail::EmptyObject => *self = Value::new_object_with(8),
            _ => {}
        }
    }

    fn unpack_static_str(&self) -> &'static str {
        debug_assert!(self.meta.get_type() == Meta::STATIC_STR);
        let ptr = unsafe { self.data.static_str.as_ptr() };
        let len = self.meta.unpack_strlen();
        unsafe { from_utf8_unchecked(from_raw_parts(ptr, len)) }
    }

    fn forward_find_shared(current: *const Value, idx: usize) -> *const Shared {
        // Navigate back from a child Value to the MetaNode at the start of the
        // bump allocation. This requires exposed provenance because `current`
        // typically has narrow provenance (from &Value indexing), but we need to
        // access memory outside that provenance range (the MetaNode before it).
        // The bump allocation's provenance was exposed in visit_root/visit_container_end.
        let meta_addr = current.expose_provenance() - idx * size_of::<Value>();
        let meta = std::ptr::with_exposed_provenance::<MetaNode>(meta_addr);
        assert!(unsafe { (*meta).canary() });
        unsafe { (*meta).shared }
    }

    fn unpack_shared(&self) -> &Shared {
        assert!(self.is_node_kind());
        unsafe {
            let idx = self.meta.unpack_dom_node().idx;
            let cur = self as *const _;
            let shared: *const Shared = Self::forward_find_shared(cur, idx as usize);
            // The shared pointer stored in MetaNode was written with full
            // provenance from the Shared allocation, so it can be dereferenced
            // directly.
            &*shared
        }
    }

    #[inline(always)]
    fn get_enum(&self) -> ValueRefInner<'_> {
        match self.unpack_ref() {
            ValueDetail::Null => ValueRefInner::Null,
            ValueDetail::Bool(b) => ValueRefInner::Bool(b),
            ValueDetail::Number(n) => ValueRefInner::Number(n.clone()),
            ValueDetail::StaticStr(s) => ValueRefInner::Str(s),
            ValueDetail::FastStr(s) => ValueRefInner::Str(s.as_str()),
            ValueDetail::RawNumFasStr(s) => ValueRefInner::RawNum(s.as_str()),
            ValueDetail::Array(a) => ValueRefInner::Array(a),
            #[cfg(not(feature = "sort_keys"))]
            ValueDetail::Object(o) => ValueRefInner::ObjectOwned(o),
            #[cfg(feature = "sort_keys")]
            ValueDetail::Object(o) => ValueRefInner::ObjectOwned(o),
            ValueDetail::Root(n) | ValueDetail::NodeInDom(n) => n.get_inner(),
            ValueDetail::EmptyArray => ValueRefInner::EmptyArray,
            ValueDetail::EmptyObject => ValueRefInner::EmptyObject,
        }
    }

    #[inline(always)]
    fn unpack_ref(&self) -> ValueDetail<'_> {
        // Safety: each arm accesses the Data union field matching the Meta type tag
        match self.meta.get_type() {
            Meta::NULL => ValueDetail::Null,
            Meta::TRUE => ValueDetail::Bool(true),
            Meta::FALSE => ValueDetail::Bool(false),
            Meta::STATIC_STR => ValueDetail::StaticStr(self.unpack_static_str()),
            Meta::I64 => ValueDetail::Number(Number::from(unsafe { self.data.ival })),
            Meta::U64 => ValueDetail::Number(Number::from(unsafe { self.data.uval })),
            Meta::F64 => ValueDetail::Number(Number::try_from(unsafe { self.data.fval }).unwrap()),
            Meta::EMPTY_ARR => ValueDetail::EmptyArray,
            Meta::EMPTY_OBJ => ValueDetail::EmptyObject,
            Meta::STR_NODE | Meta::RAWNUM_NODE | Meta::ARR_NODE | Meta::OBJ_NODE => {
                ValueDetail::NodeInDom(NodeInDom {
                    node: self,
                    dom: self.unpack_shared(),
                })
            }
            Meta::FASTSTR => ValueDetail::FastStr(unsafe { &self.data.str_own }),
            Meta::RAWNUM_FASTSTR => ValueDetail::RawNumFasStr(unsafe { &self.data.str_own }),
            Meta::ARR_MUT => ValueDetail::Array(unsafe { &self.data.arr_own }),
            Meta::OBJ_MUT => ValueDetail::Object(unsafe { &self.data.obj_own }),
            Meta::ROOT_NODE => ValueDetail::Root(NodeInDom {
                node: unsafe { self.data.root.as_ref() },
                dom: unsafe { &*self.meta.unpack_root() },
            }),
            _ => unreachable!("unknown type"),
        }
    }
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
    /// ```
    fn clone(&self) -> Self {
        match self.unpack_ref() {
            ValueDetail::Root(indom) | ValueDetail::NodeInDom(indom) => Value::from(indom),
            ValueDetail::Null => Value::new_null(),
            ValueDetail::Bool(b) => Value::new_bool(b),
            ValueDetail::Number(n) => n.into(),
            ValueDetail::StaticStr(s) => Value::from_static_str(s),
            ValueDetail::FastStr(s) => s.into(),
            ValueDetail::RawNumFasStr(s) => Value::new_rawnum_faststr(s),
            ValueDetail::Array(a) => a.clone().into(),
            ValueDetail::Object(o) => o.clone().into(),
            ValueDetail::EmptyArray => Value::new_array(),
            ValueDetail::EmptyObject => Value::new_object(),
        }
    }
}

impl From<Arc<Vec<Value>>> for Value {
    fn from(value: Arc<Vec<Value>>) -> Self {
        Self {
            meta: Meta::new(Meta::ARR_MUT),
            data: Data {
                arr_own: ManuallyDrop::new(value),
            },
        }
    }
}

#[cfg(not(feature = "sort_keys"))]
impl From<Arc<AHashMap<FastStr, Value>>> for Value {
    fn from(value: Arc<AHashMap<FastStr, Value>>) -> Self {
        Self {
            meta: Meta::new(Meta::OBJ_MUT),
            data: Data {
                obj_own: ManuallyDrop::new(value),
            },
        }
    }
}

#[cfg(feature = "sort_keys")]
impl From<Arc<BTreeMap<FastStr, Value>>> for Value {
    fn from(value: Arc<BTreeMap<FastStr, Value>>) -> Self {
        Self {
            meta: Meta::new(Meta::OBJ_MUT),
            data: Data {
                obj_own: ManuallyDrop::new(value),
            },
        }
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_ref2())?;
        Ok(())
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::new()
    }
}

impl Value {
    /// Convert into `Object`. If the value is not an object, return `None`.
    #[inline]
    pub fn into_object(self) -> Option<Object> {
        if self.is_object() {
            Some(Object(self))
        } else {
            None
        }
    }

    /// Convert into `Array`. If the value is not an array, return `None`.
    #[inline]
    pub fn into_array(self) -> Option<Array> {
        if self.is_array() {
            Some(Array(self))
        } else {
            None
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", crate::to_string(self).expect("invalid value"))
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Safety: reading `parent` (u64) from the union is valid for any bit pattern
        let parent = unsafe { self.parent };
        match parent {
            0 => write!(f, "parent: null"),
            _ => write!(f, "parent: {parent}"),
        }
    }
}

impl super::value_trait::JsonValueTrait for Value {
    type ValueType<'v>
        = &'v Value
    where
        Self: 'v;

    #[inline]
    fn get_type(&self) -> JsonType {
        let typ = match self.get_enum() {
            ValueRefInner::Null => JsonType::Null,
            ValueRefInner::Bool(_) => JsonType::Boolean,
            ValueRefInner::Number(_) => JsonType::Number,
            ValueRefInner::Str(_) => JsonType::String,
            ValueRefInner::Array(_) => JsonType::Array,
            ValueRefInner::Object(_) | ValueRefInner::ObjectOwned(_) => JsonType::Object,
            ValueRefInner::RawNum(_) => JsonType::Number,
            ValueRefInner::EmptyArray => JsonType::Array,
            ValueRefInner::EmptyObject => JsonType::Object,
        };
        typ
    }

    #[inline]
    fn as_number(&self) -> Option<Number> {
        match self.get_enum() {
            ValueRefInner::Number(s) => Some(s),
            ValueRefInner::RawNum(s) => crate::from_str(s).ok(),
            _ => None,
        }
    }

    fn as_raw_number(&self) -> Option<RawNumber> {
        match self.unpack_ref() {
            ValueDetail::RawNumFasStr(s) => Some(RawNumber::from_faststr(s.clone())),
            ValueDetail::NodeInDom(indom) | ValueDetail::Root(indom) => match indom.get_inner() {
                ValueRefInner::RawNum(s) => Some(RawNumber::new(s)),
                _ => None,
            },
            _ => None,
        }
    }

    #[inline]
    fn as_i64(&self) -> Option<i64> {
        self.as_number().and_then(|num| num.as_i64())
    }

    #[inline]
    fn as_u64(&self) -> Option<u64> {
        self.as_number().and_then(|num| num.as_u64())
    }

    #[inline]
    fn as_f64(&self) -> Option<f64> {
        self.as_number().and_then(|num| num.as_f64())
    }

    #[inline]
    fn as_bool(&self) -> Option<bool> {
        match self.meta.get_type() {
            Meta::TRUE => Some(true),
            Meta::FALSE => Some(false),
            _ => None,
        }
    }

    #[inline]
    fn as_str(&self) -> Option<&str> {
        match self.as_ref2() {
            ValueRefInner::Str(s) => Some(s),
            _ => None,
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
            Some(Self::ArrayType::ref_cast(self))
        } else {
            None
        }
    }

    #[inline]
    fn as_object(&self) -> Option<&Self::ObjectType> {
        if self.is_object() {
            Some(Self::ObjectType::ref_cast(self))
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
            self.to_mut();
            Some(Self::ObjectType::ref_cast_mut(self))
        } else {
            None
        }
    }

    #[inline]
    fn as_array_mut(&mut self) -> Option<&mut Self::ArrayType> {
        if self.is_array() {
            self.to_mut();
            Some(Self::ArrayType::ref_cast_mut(self))
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

impl Value {
    const PADDING_SIZE: usize = 64;
    pub(crate) const HEAD_NODE_COUNT: usize = 1;

    /// Create a new `null` Value. It is also the default value of `Value`.
    #[inline]
    pub const fn new() -> Self {
        Value {
            // without shared allocator
            meta: Meta::new(Meta::NULL),
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
    #[inline]
    pub fn as_ref(&self) -> ValueRef<'_> {
        match self.get_enum() {
            ValueRefInner::Null => ValueRef::Null,
            ValueRefInner::Bool(b) => ValueRef::Bool(b),
            ValueRefInner::Number(n) => ValueRef::Number(n),
            ValueRefInner::Str(s) => ValueRef::String(s),
            ValueRefInner::Array(_) | ValueRefInner::EmptyArray => {
                ValueRef::Array(self.as_array().unwrap())
            }
            ValueRefInner::Object(_)
            | ValueRefInner::EmptyObject
            | ValueRefInner::ObjectOwned(_) => ValueRef::Object(self.as_object().unwrap()),
            ValueRefInner::RawNum(raw) => {
                crate::from_str(raw).map_or(ValueRef::Null, ValueRef::Number)
            }
        }
    }

    #[inline]
    pub(crate) fn as_ref2(&self) -> ValueRefInner<'_> {
        self.get_enum()
    }

    /// Create a new string Value from a `&'static str` with zero-copy.
    ///
    /// # Example
    /// ```
    /// use sonic_rs::{array, JsonValueTrait, Value};
    ///
    /// let s = "hello";
    /// let v1 = Value::from_static_str("hello");
    /// assert_eq!(v1.as_str().unwrap().as_ptr(), s.as_ptr());
    ///
    /// let v2 = v1.clone();
    /// assert_eq!(v1.as_str().unwrap().as_ptr(), v2.as_str().unwrap().as_ptr());
    /// ```
    #[inline]
    pub fn from_static_str(val: &'static str) -> Self {
        if val.len() >= (u32::MAX as usize) {
            return Value {
                meta: Meta::new(Meta::FASTSTR),
                data: Data {
                    str_own: ManuallyDrop::new(Box::new(FastStr::new(val))),
                },
            };
        }

        Value {
            meta: Meta::pack_static_str(Meta::STATIC_STR, val.len()),
            data: Data {
                static_str: NonNull::new(val.as_ptr() as *mut u8)
                    .expect("str::as_ptr() is non-null"),
            },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_u64(val: u64) -> Self {
        Value {
            meta: Meta::new(Meta::U64),
            data: Data { uval: val },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_i64(ival: i64) -> Self {
        Value {
            meta: Meta::new(Meta::I64),
            data: Data { ival },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub(crate) fn new_f64_unchecked(fval: f64) -> Self {
        debug_assert!(fval.is_finite(), "f64 must be finite");
        Value {
            meta: Meta::new(Meta::F64),
            data: Data { fval },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_f64(fval: f64) -> Option<Self> {
        if fval.is_finite() {
            Some(Value {
                meta: Meta::new(Meta::F64),
                data: Data { fval },
            })
        } else {
            None
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_null() -> Self {
        Value {
            meta: Meta::new(Meta::NULL),
            data: Data { uval: 0 },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub const fn new_array() -> Self {
        Value {
            meta: Meta::new(Meta::EMPTY_ARR),
            data: Data { uval: 0 },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub const fn new_object() -> Self {
        Value {
            meta: Meta::new(Meta::EMPTY_OBJ),
            data: Data { uval: 0 },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_array_with(capacity: usize) -> Self {
        let arr_own = ManuallyDrop::new(Arc::new(Vec::<Value>::with_capacity(capacity)));
        Value {
            meta: Meta::new(Meta::ARR_MUT),
            data: Data { arr_own },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_bool(val: bool) -> Self {
        Value {
            meta: Meta::new(if val { Meta::TRUE } else { Meta::FALSE }),
            data: Data { uval: 0 },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn pack_str(kind: u64, idx: usize, val: &str) -> Self {
        let node_idx = idx as u32;
        // we check the json length when parsing, so val.len() should always be less than u32::MAX
        Value {
            meta: Meta::pack_dom_node(kind, node_idx, val.len() as u32),
            data: Data {
                dom_str: NonNull::new(val.as_ptr() as *mut _).expect("str::as_ptr() is non-null"),
            },
        }
    }

    #[inline]
    pub(crate) fn new_rawnum_faststr(num: &FastStr) -> Self {
        let str_own = ManuallyDrop::new(Box::new(num.clone()));
        Value {
            meta: Meta::new(Meta::RAWNUM_FASTSTR),
            data: Data { str_own },
        }
    }

    #[inline]
    pub(crate) fn new_rawnum(num: &str) -> Self {
        let str_own = ManuallyDrop::new(Box::new(FastStr::new(num)));
        Value {
            meta: Meta::new(Meta::RAWNUM_FASTSTR),
            data: Data { str_own },
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self.as_ref2() {
            ValueRefInner::Array(arr) => arr.len(),
            ValueRefInner::Object(obj) => obj.len(),
            ValueRefInner::Str(s) => s.len(),
            _ => 0,
        }
    }

    pub(crate) fn as_value_slice(&self) -> Option<&[Value]> {
        match self.as_ref2() {
            ValueRefInner::Array(s) => Some(s),
            ValueRefInner::EmptyArray => Some(&[]),
            _ => None,
        }
    }

    pub(crate) fn as_obj_len(&self) -> usize {
        match self.as_ref2() {
            ValueRefInner::Object(s) => s.len(),
            ValueRefInner::EmptyObject => 0,
            ValueRefInner::ObjectOwned(s) => s.len(),
            _ => unreachable!("value is not object"),
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn copy_str(val: &str) -> Self {
        let str_own = ManuallyDrop::new(Box::new(FastStr::new(val)));
        Value {
            meta: Meta::new(Meta::FASTSTR),
            data: Data { str_own },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn copy_str_in(kind: u64, val: &str, idx: usize, shared: &mut Shared) -> Self {
        let str = shared.get_alloc().alloc_str(val);
        let node_idx = idx as u32;
        // we check the json length when parsing, so val.len() should always be less than u32::MAX
        Value {
            meta: Meta::pack_dom_node(kind, node_idx, str.len() as u32),
            data: Data {
                dom_str: NonNull::new(str.as_ptr() as *mut _).expect("str::as_ptr() is non-null"),
            },
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn new_faststr(val: FastStr) -> Self {
        let str_own = ManuallyDrop::new(Box::new(val));
        Value {
            meta: Meta::new(Meta::FASTSTR),
            data: Data { str_own },
        }
    }

    #[doc(hidden)]
    pub fn new_object_with(
        #[cfg(not(feature = "sort_keys"))] capacity: usize,
        #[cfg(feature = "sort_keys")] _: usize,
    ) -> Self {
        let obj_own = ManuallyDrop::new(Arc::new(
            #[cfg(not(feature = "sort_keys"))]
            AHashMap::with_capacity(capacity),
            #[cfg(feature = "sort_keys")]
            BTreeMap::new(),
        ));
        Value {
            meta: Meta::new(Meta::OBJ_MUT),
            data: Data { obj_own },
        }
    }

    pub(crate) fn get_index(&self, index: usize) -> Option<&Self> {
        debug_assert!(self.is_array(), "{self:?}");
        if let ValueRefInner::Array(s) = self.as_ref2() {
            if index < s.len() {
                return Some(&s[index]);
            }
        }
        None
    }

    pub(crate) fn get_index_mut(&mut self, index: usize) -> Option<&mut Self> {
        debug_assert!(self.is_array());
        if let ValueMut::Array(s) = self.as_mut() {
            if index < s.len() {
                return Some(&mut s[index]);
            }
        }
        None
    }

    #[inline]
    pub(crate) fn get_key(&self, key: &str) -> Option<&Self> {
        self.get_key_value(key).map(|(_, v)| v)
    }

    pub(crate) fn get_key_value(&self, key: &str) -> Option<(&str, &Self)> {
        debug_assert!(self.is_object());
        let ref_inner = self.as_ref2();
        if let ValueRefInner::Object(kv) = ref_inner {
            for (k, v) in kv {
                let k = k.as_str().expect("key is not string");
                if k == key {
                    return Some((k, v));
                }
            }
        } else if let ValueRefInner::ObjectOwned(kv) = ref_inner {
            if let Some((k, v)) = kv.get_key_value(key) {
                return Some((k.as_str(), v));
            }
        }
        None
    }

    #[inline]
    pub(crate) fn get_key_mut(&mut self, key: &str) -> Option<&mut Self> {
        if let ValueMut::Object(kv) = self.as_mut() {
            if let Some(v) = kv.get_mut(key) {
                return Some(v);
            }
        }
        None
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        debug_assert!(self.is_object() || self.is_array());
        match self.unpack_ref() {
            ValueDetail::Array(arr) => arr.capacity(),
            #[cfg(not(feature = "sort_keys"))]
            ValueDetail::Object(obj) => obj.capacity(),
            #[cfg(feature = "sort_keys")]
            ValueDetail::Object(obj) => obj.len(),
            ValueDetail::NodeInDom(indom) | ValueDetail::Root(indom) => {
                if self.is_object() {
                    indom.unpack_pair_slice().len()
                } else {
                    indom.unpack_value_slice().len()
                }
            }
            ValueDetail::EmptyArray | ValueDetail::EmptyObject => 0,
            _ => unreachable!("value is not array or object"),
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        debug_assert!(self.is_object() || self.is_array());
        match self.as_mut() {
            ValueMut::Array(arr) => arr.clear(),
            ValueMut::Object(obj) => obj.clear(),
            _ => unreachable!("value is not array or object"),
        }
    }

    #[inline]
    pub(crate) fn remove_index(&mut self, index: usize) -> Value {
        debug_assert!(self.is_array());
        match self.as_mut() {
            ValueMut::Array(arr) => arr.remove(index),
            _ => unreachable!("value is not array"),
        }
    }

    #[inline]
    pub(crate) fn remove_key(&mut self, k: &str) -> Option<Value> {
        debug_assert!(self.is_object());
        match self.as_mut() {
            ValueMut::Object(obj) => obj.remove(k),
            _ => unreachable!("value is not object"),
        }
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
        std::mem::take(self)
    }

    #[inline]
    pub(crate) fn reserve<T>(&mut self, additional: usize) {
        debug_assert!(self.is_object() || self.is_array());
        debug_assert!(size_of::<T>() == size_of::<Value>() || size_of::<T>() == size_of::<Pair>());
        match self.as_mut() {
            ValueMut::Array(arr) => arr.reserve(additional),
            #[cfg(not(feature = "sort_keys"))]
            ValueMut::Object(obj) => obj.reserve(additional),
            #[cfg(feature = "sort_keys")]
            ValueMut::Object(_) => {}
            _ => unreachable!("value is not array or object"),
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn append_value(&mut self, val: Value) -> &mut Value {
        debug_assert!(self.is_array());
        match self.as_mut() {
            ValueMut::Array(arr) => {
                arr.push(val);
                let len = arr.len();
                &mut arr[len - 1]
            }
            _ => unreachable!("value is not array"),
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn insert(&mut self, key: &str, val: Value) -> &mut Value {
        debug_assert!(self.is_object());
        match self.as_mut() {
            ValueMut::Object(obj) => {
                obj.insert(FastStr::new(key), val);
                obj.get_mut(key).unwrap()
            }
            _ => unreachable!("value is not object"),
        }
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<Value> {
        debug_assert!(self.is_array());
        match self.as_mut() {
            ValueMut::Array(arr) => arr.pop(),
            _ => unreachable!("value is not object"),
        }
    }

    #[inline(never)]
    pub(crate) fn parse_with_padding(&mut self, json: &[u8], cfg: DeserializeCfg) -> Result<usize> {
        // allocate the padding buffer for the input json
        let mut shared = Arc::new(Shared::default());
        // Expose Arc allocation provenance (header + data) so that
        // Arc::increment_strong_count in pack_shared can recover it
        // via with_exposed_provenance.
        Arc::as_ptr(&shared).expose_provenance();
        let mut buffer = Vec::with_capacity(json.len() + Self::PADDING_SIZE);
        buffer.extend_from_slice(json);
        buffer.extend_from_slice(&b"x\"x"[..]);
        buffer.extend_from_slice(&[0; 61]);

        let smut = Arc::get_mut(&mut shared).unwrap();
        let slice = PaddedSliceRead::new(buffer.as_mut_slice(), json);
        let mut parser = Parser::new(slice).with_config(cfg);
        let mut vis = DocumentVisitor::new(json.len(), smut);
        parser.parse_dom(&mut vis, None)?;
        let idx = parser.read.index();

        // NOTE: root node should is the first node
        *self = unsafe { vis.root.as_ref().clone() };
        smut.set_json(buffer);
        Ok(idx)
    }

    #[inline(never)]
    pub(crate) fn parse_without_padding<'de, R: Reader<'de>>(
        &mut self,
        shared: &mut Shared,
        strbuf: &mut Vec<u8>,
        parser: &mut Parser<R>,
    ) -> Result<()> {
        let remain_len = parser.read.remain();
        let mut vis = DocumentVisitor::new(remain_len, shared);
        parser.parse_dom(&mut vis, Some(strbuf))?;
        *self = unsafe { vis.root.as_ref().clone() };
        Ok(())
    }
}

pub(crate) struct DocumentVisitor<'a> {
    pub(crate) shared: *mut Shared,
    pub(crate) nodes: NodeBuf,
    pub(crate) parent: usize,
    pub(crate) nodes_start: usize,
    pub(crate) root: NonNull<Value>,
    _marker: std::marker::PhantomData<&'a mut Shared>,
}

impl<'a> DocumentVisitor<'a> {
    fn new(json_len: usize, shared: &'a mut Shared) -> Self {
        let max_len = (json_len / 2) + 2;
        let nodes = NodeBuf::with_capacity(max_len);
        let shared = shared as *mut Shared;
        (shared as *const Shared).expose_provenance();
        DocumentVisitor {
            shared,
            nodes,
            parent: 0,
            nodes_start: 0,
            root: NonNull::dangling(),
            _marker: std::marker::PhantomData,
        }
    }

    #[inline(always)]
    fn nodes_len(&self) -> usize {
        self.nodes.len()
    }

    #[inline(always)]
    fn index(&self) -> usize {
        self.nodes_len() - self.parent
    }
}

#[repr(C)]
struct MetaNode {
    shared: *const Shared,
    canary: u64,
}

const _: () = assert!(
    std::mem::size_of::<MetaNode>() == std::mem::size_of::<Value>(),
    "MetaNode and Value must have the same size for transmute safety"
);

impl MetaNode {
    fn new(shared: *const Shared) -> Self {
        let canary = b"SONICRS\0";
        MetaNode {
            shared,
            canary: u64::from_ne_bytes(*canary),
        }
    }

    fn canary(&self) -> bool {
        self.canary == u64::from_ne_bytes(*b"SONICRS\0")
    }
}

impl<'a> DocumentVisitor<'a> {
    fn visit_container_start(&mut self, kind: u64) -> bool {
        let ret = self.push_node(Value {
            meta: Meta::pack_dom_node(kind, 0, 0), // update when array ending
            data: Data {
                parent: self.parent as u64, // record the old parent offset
            },
        });
        self.parent = self.nodes_len() - 1;
        ret
    }

    // the array and object's logic is same.
    #[inline(always)]
    fn visit_container_end(&mut self, kind: u64, len: usize) -> bool {
        let parent = self.parent;
        let old = unsafe { self.nodes.node_ref(parent).data.parent as usize };

        self.parent = old;
        if len == 0 {
            self.nodes.node_mut(parent).meta = Meta::new(if kind == Meta::OBJ_NODE {
                Meta::EMPTY_OBJ
            } else {
                Meta::EMPTY_ARR
            });
            return true;
        }
        unsafe {
            let children_count = self.nodes_len() - (parent + 1);
            let real_count = children_count + Value::HEAD_NODE_COUNT;
            let layout = Layout::array::<Value>(real_count).unwrap();
            let hdr = (*self.shared).get_alloc().alloc_layout(layout).as_ptr()
                as *mut ManuallyDrop<Value>;

            (hdr as *const ManuallyDrop<Value>).expose_provenance();

            let elems = hdr.add(Value::HEAD_NODE_COUNT);
            self.nodes.copy_to(parent + 1, elems, children_count);

            let meta = &mut *(hdr as *mut MetaNode);
            meta.shared = self.shared as *const _;
            meta.canary = u64::from_ne_bytes(*b"SONICRS\0");

            let idx = (parent - self.parent) as u32;
            let container = self.nodes.node_mut(parent);
            container.meta = Meta::pack_dom_node(kind, idx, len as u32);
            container.data.arr_elems = NonNull::new_unchecked(elems as *mut _);
            self.nodes.truncate(parent + 1);
        }
        true
    }

    fn visit_root(&mut self) {
        // should alloc root node in the bump allocator
        let start = self.nodes_start;
        let ptr = self.shared as *const Shared;
        let tuple_ref =
            unsafe { (*self.shared).get_alloc() }.alloc((MetaNode::new(ptr), Value::default()));

        // Expose provenance of the full (MetaNode, Value) allocation so that
        // forward_find_shared can navigate back to MetaNode via with_exposed_provenance.
        (tuple_ref as *const (MetaNode, Value)).expose_provenance();

        // Copy source node to root using ptr::copy to preserve pointer provenance
        // in the Data union. Copying through data.uval (u64) would strip provenance.
        let src = self.nodes.node_ref(start) as *const ManuallyDrop<Value> as *const Value;
        let dst = &mut tuple_ref.1 as *mut Value;
        unsafe { std::ptr::copy_nonoverlapping(src, dst, 1) };
        self.root = unsafe { NonNull::new_unchecked(dst) };
    }

    /// Push a node. Production: pointer advancement (avoids STLF stalls).
    /// Miri: Vec::push (preserves provenance).
    #[inline(always)]
    fn push_node(&mut self, node: Value) -> bool {
        self.push_raw(ManuallyDrop::new(node))
    }

    #[inline(always)]
    fn push_meta(&mut self, node: MetaNode) -> bool {
        self.push_raw(ManuallyDrop::new(unsafe {
            transmute::<MetaNode, Value>(node)
        }))
    }

    #[inline(always)]
    fn push_raw(&mut self, val: ManuallyDrop<Value>) -> bool {
        self.nodes.push(val)
    }
}

impl<'de, 'a> JsonVisitor<'de> for DocumentVisitor<'a> {
    #[inline(always)]
    fn visit_dom_start(&mut self) -> bool {
        let shared = self.shared as *const Shared;
        self.push_meta(MetaNode::new(shared));
        self.nodes_start = self.nodes_len();
        assert_eq!(self.nodes_len(), 1);
        true
    }

    #[inline(always)]
    fn visit_bool(&mut self, val: bool) -> bool {
        self.push_node(Value::new_bool(val))
    }

    #[inline(always)]
    fn visit_f64(&mut self, val: f64) -> bool {
        let node = Value::new_f64_unchecked(val);
        self.push_node(node)
    }

    #[inline(always)]
    fn visit_raw_number(&mut self, val: &str) -> bool {
        let idx = self.index();
        let node = Value::copy_str_in(Meta::RAWNUM_NODE, val, idx, unsafe { &mut *self.shared });
        self.push_node(node)
    }

    #[inline(always)]
    fn visit_borrowed_raw_number(&mut self, val: &str) -> bool {
        let idx = self.index();
        self.push_node(Value::pack_str(Meta::RAWNUM_NODE, idx, val))
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
        self.visit_container_start(Meta::ARR_NODE)
    }

    #[inline(always)]
    fn visit_array_end(&mut self, len: usize) -> bool {
        self.visit_container_end(Meta::ARR_NODE, len)
    }

    #[inline(always)]
    fn visit_object_start(&mut self, _hint: usize) -> bool {
        self.visit_container_start(Meta::OBJ_NODE)
    }

    #[inline(always)]
    fn visit_object_end(&mut self, len: usize) -> bool {
        self.visit_container_end(Meta::OBJ_NODE, len)
    }

    #[inline(always)]
    fn visit_null(&mut self) -> bool {
        self.push_node(Value::new_null())
    }

    // this api should never used for parsing with padding buffer
    #[inline(always)]
    fn visit_str(&mut self, val: &str) -> bool {
        let idx = self.index();
        let node = Value::copy_str_in(Meta::STR_NODE, val, idx, unsafe { &mut *self.shared });
        self.push_node(node)
    }

    #[inline(always)]
    fn visit_borrowed_str(&mut self, val: &'de str) -> bool {
        let idx = self.index();
        self.push_node(Value::pack_str(Meta::STR_NODE, idx, val))
    }

    #[inline(always)]
    fn visit_key(&mut self, key: &str) -> bool {
        self.visit_str(key)
    }

    #[inline(always)]
    fn visit_borrowed_key(&mut self, key: &'de str) -> bool {
        self.visit_borrowed_str(key)
    }

    fn visit_dom_end(&mut self) -> bool {
        self.visit_root();
        true
    }
}

impl Serialize for Value {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        match self.as_ref2() {
            ValueRefInner::Null => serializer.serialize_unit(),
            ValueRefInner::Bool(b) => serializer.serialize_bool(b),
            ValueRefInner::Number(n) => n.serialize(serializer),
            ValueRefInner::Str(s) => s.serialize(serializer),
            ValueRefInner::Array(a) => {
                let mut seq = tri!(serializer.serialize_seq(Some(a.len())));
                for n in a {
                    tri!(seq.serialize_element(n));
                }
                seq.end()
            }
            ValueRefInner::EmptyArray => serializer.serialize_seq(None)?.end(),
            ValueRefInner::EmptyObject => serializer.serialize_map(None)?.end(),
            ValueRefInner::Object(o) => {
                #[cfg(feature = "sort_keys")]
                {
                    // TODO: sort the keys use thread-local buffer
                    let mut kvs: Vec<&(Value, Value)> = o.iter().collect();
                    kvs.sort_by(|(k1, _), (k2, _)| k1.as_str().unwrap().cmp(k2.as_str().unwrap()));
                    let mut map = tri!(serializer.serialize_map(Some(kvs.len())));
                    for (k, v) in kvs {
                        tri!(map.serialize_key(k.as_str().unwrap()));
                        tri!(map.serialize_value(v));
                    }
                    map.end()
                }
                #[cfg(not(feature = "sort_keys"))]
                {
                    let entries = o.iter();
                    let mut map = tri!(serializer.serialize_map(Some(entries.len())));
                    for (k, v) in entries {
                        tri!(map.serialize_key(k.as_str().unwrap()));
                        tri!(map.serialize_value(v));
                    }
                    map.end()
                }
            }
            #[cfg(not(feature = "sort_keys"))]
            ValueRefInner::ObjectOwned(o) => {
                let mut map = tri!(serializer.serialize_map(Some(o.len())));
                for (k, v) in o.iter() {
                    tri!(map.serialize_key(k.as_str()));
                    tri!(map.serialize_value(v));
                }
                map.end()
            }
            #[cfg(feature = "sort_keys")]
            ValueRefInner::ObjectOwned(o) => {
                let mut map = tri!(serializer.serialize_map(Some(o.len())));
                for (k, v) in o.iter() {
                    tri!(map.serialize_key(k.as_str()));
                    tri!(map.serialize_value(v));
                }
                map.end()
            }
            ValueRefInner::RawNum(raw) => {
                use serde::ser::SerializeStruct;

                use crate::serde::rawnumber::TOKEN;
                let mut struct_ = tri!(serializer.serialize_struct(TOKEN, 1));
                tri!(struct_.serialize_field(TOKEN, raw));
                struct_.end()
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[cfg(feature = "sort_keys")]
    use crate::object;
    use crate::{error::make_error, from_slice, from_str, pointer, util::mock::MockString};

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct ValueInStruct {
        val: Value,
    }

    fn test_value_instruct(data: &str) -> Result<()> {
        if let Ok(val) = from_str::<Value>(data) {
            let valin = ValueInStruct { val: val.clone() };
            let out = crate::to_string(&valin)?;
            let valin2: ValueInStruct = from_str(&out).unwrap();
            if valin2.val != val {
                diff_json(data);
                return Err(make_error(format!(
                    "invalid result when test parse valid json to ValueInStruct {data}"
                )));
            }
        }
        Ok(())
    }

    fn test_value(data: &str) -> Result<()> {
        let serde_value: serde_json::Result<serde_json::Value> = serde_json::from_str(data);
        let dom: Result<Value> = from_slice(data.as_bytes());

        if let Ok(serde_value) = serde_value {
            let dom = dom.unwrap();
            let sonic_out = crate::to_string(&dom)?;
            let serde_value2: serde_json::Value = serde_json::from_str(&sonic_out).unwrap();

            if serde_value == serde_value2 {
                test_value_instruct(data)?;
                Ok(())
            } else {
                diff_json(data);
                Err(make_error(format!("invalid result for valid json {data}")))
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

    #[cfg(not(target_arch = "wasm32"))]
    fn test_value_file(path: &std::path::Path) {
        let data = std::fs::read_to_string(path).unwrap();
        assert!(test_value(&data).is_ok(), "failed json is {path:?}");
    }

    #[test]
    fn test_node_basic() {
        // // Valid JSON object
        // let data = r#"{"name": "John", "age": 30}"#;
        // assert!(test_value(data).is_ok(), "failed json is {}", data);

        // // Valid JSON array
        // let data = r#"[1, 2, 3]"#;
        // assert!(test_value(data).is_ok(), "failed json is {}", data);

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
        assert!(test_value(data).is_ok(), "failed json is {data}");

        // // Valid JSON data with escape characters
        // let data = r#"{
        //     "name": "John",
        //     "age": 30,
        //     "description": "He said, \"I'm coming home.\""
        // }"#;
        // assert!(test_value(data).is_ok(), "failed json is {}", data);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(miri))]
    fn test_node_from_files3() {
        use std::fs::DirEntry;
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/benchmarks/benches/testdata/";
        println!("dir is {path}");

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
            let file_size = file.metadata().unwrap().len();
            if path.extension().unwrap_or_default() == "json"
                && !path.ends_with("canada.json")
                && file_size < 500_000
            {
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
            assert!(ret.is_err(), "failed json is {data}");
        }
    }

    #[test]
    fn test_parse_numbrs() {
        let testdata = [
            " 33.3333333043333333",
            " 33.3333333053333333 ",
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

    #[cfg(not(feature = "utf8_lossy"))]
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

        assert_eq!(value.pointer(pointer!["array", 2]).as_u64().unwrap(), 3);
        assert_eq!(
            value.pointer(pointer!["object", "a"]).as_str().unwrap(),
            "aaa"
        );
        assert_eq!(value.pointer(pointer!["objempty", "a"]).as_str(), None);

        assert_eq!(value.pointer(pointer!["arrempty", 1]).as_str(), None);

        assert!(!value.pointer(pointer!["unknown"]).is_str());
    }

    #[cfg(not(feature = "utf8_lossy"))]
    #[test]
    fn test_invalid_utf8() {
        use crate::{from_slice, from_slice_unchecked};

        let data = [b'"', 0x80, 0x90, b'"'];
        let ret: Result<Value> = from_slice(&data);
        assert_eq!(
            ret.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 2\n\n\t\"��\"\n\t.^..\n"
        );

        let dom: Result<Value> = unsafe { from_slice_unchecked(&data) };
        assert!(dom.is_ok(), "{}", dom.unwrap_err());

        let data = [b'"', b'"', 0x80];
        let dom: Result<Value> = from_slice(&data);
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 3\n\n\t\"\"�\n\t..^\n"
        );

        let data = [0x80, b'"', b'"'];
        let dom: Result<Value> = unsafe { from_slice_unchecked(&data) };
        assert_eq!(
            dom.err().unwrap().to_string(),
            "Invalid JSON value at line 1 column 1\n\n\t�\"\"\n\t^..\n"
        );
    }

    #[test]
    fn test_value_serde() {
        use serde::{Deserialize, Serialize};

        use crate::{array, object};
        #[derive(Deserialize, Debug, Serialize, PartialEq)]
        struct Foo {
            value: Value,
            object: Object,
            array: Array,
        }

        let foo: Foo = crate::from_str(&MockString::from(
            r#"
        {
            "value": "hello",
            "object": {"a": "b"},
            "array": [1,2,3]
        }"#,
        ))
        .unwrap();

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

    #[test]
    #[cfg(not(miri))]
    fn test_arbitrary_precision() {
        use crate::Deserializer;

        let nums = [
            "-46333333333333333333333333333333.6",
            "43.420273000",
            "1e123",
            "0.001","0e+12","0.1e+12",
            "0", "0.0", "1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345e+1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345",
            "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123",
         "1.23456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567e89012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123",
        "-0.000000023456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567e+89012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123",
        ];

        for num in nums {
            let mut de = Deserializer::from_str(num).use_rawnumber();
            let value: Value = de.deserialize().unwrap();
            assert_eq!(value.as_raw_number().unwrap().as_str(), num);
            assert_eq!(value.to_string(), num);
        }
    }

    #[cfg(feature = "sort_keys")]
    #[test]
    fn test_sort_keys() {
        struct Case<'a> {
            input: &'a str,
            output: &'a str,
        }

        let cases = [
            Case {
                input: r#"{"b": 2,"bc":{"cb":1,"ca":"hello"},"a": 1}"#,
                output: r#"{"a":1,"b":2,"bc":{"ca":"hello","cb":1}}"#,
            },
            Case {
                input: r#"{"a":1}"#,
                output: r#"{"a":1}"#,
            },
            Case {
                input: r#"{"b": 2,"a": 1}"#,
                output: r#"{"a":1,"b":2}"#,
            },
            Case {
                input: "{}",
                output: "{}",
            },
            Case {
                input: r#"[{"b": 2,"c":{"cb":1,"ca":"hello"},"a": 1}, {"ab": 2,"aa": 1}]"#,
                output: r#"[{"a":1,"b":2,"c":{"ca":"hello","cb":1}},{"aa":1,"ab":2}]"#,
            },
        ];

        for case in cases {
            let value: Value = crate::from_str(case.input).unwrap();
            assert_eq!(value.to_string(), case.output);
        }
    }

    #[cfg(feature = "sort_keys")]
    #[test]
    fn test_sort_keys_owned() {
        let obj = object! {
            "b": 2,
            "bc": object! {
                "cb": 1,
                "ca": "hello",
            },
            "a": 1,
        };

        let obj2 = object! {
            "a": 1,
            "b": 2,
            "bc": object! {
                "ca": "hello",
                "cb": 1,
            },
        };

        assert_eq!(obj, obj2);
    }

    #[test]
    fn test_issue_179_line_column() {
        let json = r#"
        {
            "key\nwith\nnewlines": "value",
            "another_key": [, 1, 2, 3]
        }
        "#;
        let err = crate::from_str::<Value>(json).unwrap_err();
        assert_eq!(err.line(), 4);
    }
}
