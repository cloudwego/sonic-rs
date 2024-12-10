use std::{
    fmt::{self, Debug, Display},
    str::from_utf8_unchecked,
    sync::atomic::{AtomicPtr, Ordering},
};

use faststr::FastStr;
use ref_cast::RefCast;
use serde::ser::{SerializeMap, SerializeStruct};

use super::value::HasEsc;
use crate::{
    index::Index, input::JsonSlice, serde::Number, JsonInput, JsonType, JsonValueTrait, LazyValue,
    RawNumber, Result,
};

/// OwnedLazyValue wrappers a unparsed raw JSON text. It is owned and support `Get, Set`
///
/// It can be converted from [`LazyValue`](crate::lazyvalue::LazyValue). It can be used for serde.
///
/// Default value is a raw JSON text `null`.
///
/// # Examples
///
/// ```
/// use sonic_rs::{get, JsonValueTrait, OwnedLazyValue};
///
/// // get a lazyvalue from a json, the "a"'s value will not be parsed
/// let input = r#"{
///  "a": "hello world",
///  "b": true,
///  "c": [0, 1, 2],
///  "d": {
///     "sonic": "rs"
///   }
/// }"#;
///
/// let own_a = OwnedLazyValue::from(get(input, &["a"]).unwrap());
/// let own_c = OwnedLazyValue::from(get(input, &["c"]).unwrap());
///
/// // use as_raw_xx to get the unparsed JSON text
/// assert_eq!(own_a.as_raw_str(), "\"hello world\"");
/// assert_eq!(own_c.as_raw_str(), "[0, 1, 2]");
///
/// // use as_xx to get the parsed value
/// assert_eq!(own_a.as_str().unwrap(), "hello world");
/// assert_eq!(own_c.as_str(), None);
/// assert!(own_c.is_array());
/// ```
///
/// # Serde Examples
///
/// ```
/// # use sonic_rs::{LazyValue, OwnedLazyValue};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Deserialize, Serialize, PartialEq)]
/// struct TestLazyValue<'a> {
///     #[serde(borrow)]
///     borrowed_lv: LazyValue<'a>,
///     owned_lv: OwnedLazyValue,
/// }
///
/// let input = r#"{ "borrowed_lv": "hello", "owned_lv": "world" }"#;
///
/// let data: TestLazyValue = sonic_rs::from_str(input).unwrap();
/// assert_eq!(data.borrowed_lv.as_raw_str(), "\"hello\"");
/// assert_eq!(data.owned_lv.as_raw_str(), "\"world\"");
/// ```
#[derive(Debug, Clone)]
pub struct OwnedLazyValue(pub(crate) LazyPacked);

impl Default for OwnedLazyValue {
    fn default() -> Self {
        Self(LazyPacked::Parsed(Parsed::Null))
    }
}

impl OwnedLazyValue {
    pub(crate) fn from_raw(raw: FastStr) -> Self {
        Self(LazyPacked::Raw(LazyRaw::new(raw)))
    }

    pub(crate) fn from_non_esc_str(raw: FastStr) -> Self {
        Self(LazyPacked::NonEscStrRaw(raw))
    }

    pub(crate) fn from_faststr(str: FastStr) -> Self {
        Self(LazyPacked::Parsed(Parsed::String(str)))
    }
}

impl From<Number> for OwnedLazyValue {
    fn from(number: Number) -> Self {
        Self(LazyPacked::Parsed(Parsed::Number(number)))
    }
}

impl From<Vec<(FastStr, OwnedLazyValue)>> for OwnedLazyValue {
    fn from(v: Vec<(FastStr, OwnedLazyValue)>) -> Self {
        Self(LazyPacked::Parsed(Parsed::LazyObject(v)))
    }
}

impl From<Vec<OwnedLazyValue>> for OwnedLazyValue {
    fn from(v: Vec<OwnedLazyValue>) -> Self {
        Self(LazyPacked::Parsed(Parsed::LazyArray(v)))
    }
}

impl From<bool> for OwnedLazyValue {
    fn from(v: bool) -> Self {
        Self(LazyPacked::Parsed(Parsed::Bool(v)))
    }
}

impl From<()> for OwnedLazyValue {
    fn from(_: ()) -> Self {
        Self(LazyPacked::Parsed(Parsed::Null))
    }
}

struct LazyRaw {
    // the raw slice from origin json
    pub(crate) raw: FastStr,
    pub(crate) parsed: AtomicPtr<Parsed>,
}

impl Debug for LazyRaw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.parsed.load(Ordering::Relaxed);
        let s = if ptr.is_null() {
            format!("<nill>")
        } else {
            format!("{:?}", unsafe { &*ptr })
        };
        f.debug_struct("LazyRaw")
            .field("raw", &self.raw)
            .field("parsed", &s)
            .finish()
    }
}

impl LazyRaw {
    fn new(raw: FastStr) -> Self {
        Self {
            raw,
            parsed: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    fn load(&self) -> Result<&Parsed> {
        let ptr = self.parsed.load(Ordering::Acquire);
        if !ptr.is_null() {
            return Ok(unsafe { &*ptr });
        }

        // let ov = crate::from_slice
        let mut parser = crate::parser::Parser::new(crate::Read::from(&self.raw));
        let mut strbuf: Vec<u8> = Vec::new();
        let olv: OwnedLazyValue = parser.load_owned_lazyvalue(&mut strbuf)?;
        let OwnedLazyValue(LazyPacked::Parsed(v)) = olv else {
            unreachable!("must be lazy parsed");
        };
        let parsed = Box::into_raw(Box::new(v));
        match self
            .parsed
            .compare_exchange_weak(ptr, parsed, Ordering::AcqRel, Ordering::Acquire)
        {
            // will free by drop
            Ok(_) => Ok(unsafe { &*parsed }),
            Err(ptr) => {
                // # Safety
                // the pointer is immutable here, and we can drop it
                unsafe { Box::from_raw(parsed) };
                return Ok(unsafe { &*ptr });
            }
        }
    }

    fn parse(&mut self) -> Result<Parsed> {
        let ptr = self.parsed.get_mut();
        if !(*ptr).is_null() {
            let v = unsafe { std::ptr::read(*ptr) };
            *ptr = std::ptr::null_mut();
            return Ok(v);
        }

        // let ov = crate::from_slice
        let mut parser = crate::parser::Parser::new(crate::Read::from(&self.raw));
        let mut strbuf: Vec<u8> = Vec::new();
        let olv: OwnedLazyValue = parser.load_owned_lazyvalue(&mut strbuf)?;
        let OwnedLazyValue(LazyPacked::Parsed(v)) = olv else {
            unreachable!("must be lazy parsed");
        };
        Ok(v)
    }

    fn get<I: Index>(&self, idx: I) -> Option<&OwnedLazyValue> {
        let parsed = self.load().ok()?;
        parsed.get(idx)
    }

    fn as_number(&self) -> Option<Number> {
        match self.load().ok()? {
            Parsed::Number(n) => Some(n.clone()),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self.load().ok()? {
            Parsed::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_raw_number(&self) -> Option<RawNumber> {
        if self.raw.as_bytes()[0] == b'-' || self.raw.as_bytes()[0].is_ascii_digit() {
            Some(RawNumber::from_faststr(self.raw.clone()))
        } else {
            None
        }
    }

    fn get_type(&self) -> JsonType {
        match self.raw.as_bytes()[0] {
            b'-' | b'0'..=b'9' => JsonType::Number,
            b'"' => JsonType::String,
            b'[' => JsonType::Array,
            b'{' => JsonType::Object,
            _ => unreachable!("invalid raw json value"),
        }
    }

    fn clone_lazyraw(&self) -> std::result::Result<LazyRaw, Parsed> {
        let parsed = self.parsed.load(Ordering::Relaxed);
        if parsed.is_null() {
            Ok(LazyRaw {
                raw: self.raw.clone(),
                parsed: AtomicPtr::new(std::ptr::null_mut()),
            })
        } else {
            // # Safety
            // the pointer is immutable here, and we can clone it
            Err(unsafe { (*parsed).clone() })
        }
    }
}

#[derive(Debug)]
pub(crate) enum LazyPacked {
    // raw value: number, maybe esc strings, raw object, raw array
    Raw(LazyRaw),
    // most JSON string without escaped chars, will also optimize serialize
    NonEscStrRaw(FastStr),
    Parsed(Parsed),
}

impl LazyPacked {}

impl Clone for LazyPacked {
    fn clone(&self) -> Self {
        match self {
            Self::Raw(raw) => match raw.clone_lazyraw() {
                Ok(raw) => Self::Raw(raw),
                Err(v) => Self::Parsed(v),
            },
            Self::NonEscStrRaw(s) => Self::NonEscStrRaw(s.clone()),
            Self::Parsed(v) => Self::Parsed(v.clone()),
        }
    }
}

#[derive(Debug, Clone)]
enum Parsed {
    LazyObject(Vec<(FastStr, OwnedLazyValue)>),
    LazyArray(Vec<OwnedLazyValue>),
    String(FastStr),
    Number(Number),
    Null,
    Bool(bool),
}

impl Parsed {
    fn get_type(&self) -> JsonType {
        match self {
            Parsed::LazyObject(_) => JsonType::Object,
            Parsed::LazyArray(_) => JsonType::Array,
            Parsed::String(_) => JsonType::String,
            Parsed::Number(_) => JsonType::Number,
            Parsed::Null => JsonType::Null,
            Parsed::Bool(_) => JsonType::Boolean,
        }
    }

    fn get<I: Index>(&self, index: I) -> Option<&OwnedLazyValue> {
        match self {
            Parsed::LazyObject(obj) => {
                if let Some(key) = index.as_key() {
                    for (k, v) in obj {
                        if k == key {
                            return Some(v);
                        }
                    }
                }
                return None;
            }
            Parsed::LazyArray(arr) => {
                if let Some(index) = index.as_index() {
                    return arr.get(index);
                } else {
                    return None;
                }
            }
            _ => None,
        }
    }

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<OwnedLazyValue>
    where
        P::Item: Index,
    {
        todo!()
    }
}

impl JsonValueTrait for OwnedLazyValue {
    type ValueType<'v> = &'v OwnedLazyValue;

    fn as_bool(&self) -> Option<bool> {
        if let LazyPacked::Parsed(Parsed::Bool(b)) = &self.0 {
            Some(*b)
        } else {
            None
        }
    }

    fn as_number(&self) -> Option<Number> {
        match &self.0 {
            LazyPacked::Parsed(Parsed::Number(n)) => Some(n.clone()),
            LazyPacked::Raw(raw) => raw.as_number(),
            _ => None,
        }
    }

    fn as_raw_number(&self) -> Option<crate::RawNumber> {
        match &self.0 {
            LazyPacked::Raw(raw) => raw.as_raw_number(),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match &self.0 {
            LazyPacked::Parsed(Parsed::String(s)) => Some(s.as_str()),
            LazyPacked::Raw(raw) => raw.as_str(),
            LazyPacked::NonEscStrRaw(raw) => {
                Some(unsafe { from_utf8_unchecked(&raw.as_bytes()[1..raw.len() - 1]) })
            }
            _ => None,
        }
    }

    fn get_type(&self) -> JsonType {
        match &self.0 {
            LazyPacked::Parsed(v) => v.get_type(),
            LazyPacked::Raw(raw) => raw.get_type(),
            LazyPacked::NonEscStrRaw(_) => JsonType::String,
        }
    }

    fn get<I: Index>(&self, index: I) -> Option<&OwnedLazyValue> {
        match &self.0 {
            LazyPacked::Parsed(v) => v.get(index),
            LazyPacked::Raw(raw) => raw.get(index),
            _ => None,
        }
    }

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<&OwnedLazyValue>
    where
        P::Item: Index,
    {
        todo!()
    }
}

impl OwnedLazyValue {
    /// Export the raw JSON text as `str`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{get, LazyValue};
    ///
    /// let lv: LazyValue = sonic_rs::get(r#"{"a": "hello world"}"#, &["a"]).unwrap();
    /// assert_eq!(lv.as_raw_str(), "\"hello world\"");
    /// ```
    pub fn as_raw_str(&self) -> &str {
        // # Safety
        // it is validate when using to_object_iter/get ...
        // if use `get_unchecked` unsafe apis, it must ensured by the user at first
        // match &self.0 {
        //     LazyPacked::Parsed(v) => v.as_raw_str(),
        //     LazyPacked::Raw(raw) => raw.raw.as_str(),
        //     LazyPacked::NonEscStrRaw(s) => s.as_str(),
        // }
        todo!()
    }

    /// Export the raw json text as faststr.
    ///
    /// # Note
    /// If the input json is not bytes or faststr, there will be a string copy.
    ///
    /// # Examples
    ///
    /// ```
    /// use faststr::FastStr;
    /// use sonic_rs::LazyValue;
    ///
    /// let lv: LazyValue = sonic_rs::get(r#"{"a": "hello world"}"#, &["a"]).unwrap();
    /// // will copy the raw_str into a new faststr
    /// assert_eq!(lv.as_raw_faststr(), "\"hello world\"");
    ///
    /// let fs = FastStr::new(r#"{"a": "hello world"}"#);
    /// let lv: LazyValue = sonic_rs::get(&fs, &["a"]).unwrap();
    /// assert_eq!(lv.as_raw_faststr(), "\"hello world\""); // zero-copy
    /// ```
    pub fn as_raw_faststr(&self) -> FastStr {
        todo!()
    }

    /// parse the json as OwnedLazyValue
    ///
    /// # Examples
    /// ```
    /// use faststr::FastStr;
    /// use sonic_rs::{JsonPointer, OwnedLazyValue};
    ///
    /// let lv = OwnedLazyValue::get_from(r#"{"a": "hello world"}"#, &["a"]).unwrap();
    /// assert_eq!(lv.as_raw_str(), "\"hello world\"");
    ///
    /// let lv = OwnedLazyValue::get_from(
    ///     &FastStr::new(r#"  {"a": "hello world"}  "#),
    ///     &JsonPointer::new(),
    /// )
    /// .unwrap();
    /// assert_eq!(lv.as_raw_str(), r#"{"a": "hello world"}"#);
    /// ```
    pub fn get_from<'de, Input, Path: IntoIterator>(json: Input, path: Path) -> Result<Self>
    where
        Input: JsonInput<'de>,
        Path::Item: Index,
    {
        let lv = crate::get(json, path)?;
        Ok(Self::from(lv))
    }

    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    pub fn as_object_mut(&mut self) -> Option<&mut LazyObject> {
        if let LazyPacked::Raw(raw) = &mut self.0 {
            let parsed = raw.parse().ok()?;
            self.0 = LazyPacked::Parsed(parsed);
        }

        if let LazyPacked::Parsed(Parsed::LazyObject(_)) = &mut self.0 {
            Some(LazyObject::ref_cast_mut(self))
        } else {
            None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut LazyArray> {
        if let LazyPacked::Raw(raw) = &mut self.0 {
            let parsed = raw.parse().ok()?;
            self.0 = LazyPacked::Parsed(parsed);
        }

        if let LazyPacked::Parsed(Parsed::LazyArray(_)) = &mut self.0 {
            Some(LazyArray::ref_cast_mut(self))
        } else {
            None
        }
    }

    /// get with index from lazyvalue
    pub(crate) fn get_index(&self, index: usize) -> Option<Self> {
        todo!()
    }

    /// get with key from lazyvalue
    pub(crate) fn get_key(&self, key: &str) -> Option<Self> {
        todo!()
    }

    pub(crate) fn new(raw: JsonSlice, status: HasEsc) -> Self {
        let raw = match raw {
            JsonSlice::Raw(r) => FastStr::new(unsafe { from_utf8_unchecked(r) }),
            JsonSlice::FastStr(f) => f.clone(),
        };

        if status == HasEsc::None {
            Self(LazyPacked::NonEscStrRaw(raw))
        } else {
            Self(LazyPacked::Raw(LazyRaw {
                raw,
                parsed: AtomicPtr::new(std::ptr::null_mut()),
            }))
        }
    }
}

impl<'de> From<LazyValue<'de>> for OwnedLazyValue {
    fn from(lv: LazyValue<'de>) -> Self {
        let raw = unsafe { lv.raw.into_faststr() };
        Self(LazyPacked::Raw(LazyRaw {
            raw,
            parsed: AtomicPtr::new(std::ptr::null_mut()),
        }))
    }
}

impl Display for OwnedLazyValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_raw_str())
    }
}

impl serde::ser::Serialize for OwnedLazyValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.0 {
            LazyPacked::Raw(raw) => {
                let raw = raw.raw.as_str();
                let mut s = serializer.serialize_struct(super::TOKEN, 1)?;
                // will directly write raw in `LazyValueStrEmitter::seriazlie_str`
                s.serialize_field(super::TOKEN, raw)?;
                s.end()
            }
            LazyPacked::NonEscStrRaw(raw) => {
                let raw = raw.as_str();
                let mut s = serializer.serialize_struct(super::TOKEN, 1)?;
                // will directly write raw in `LazyValueStrEmitter::seriazlie_str`
                s.serialize_field(super::TOKEN, raw)?;
                s.end()
            }
            LazyPacked::Parsed(Parsed::LazyObject(vec)) => {
                #[cfg(feature = "sort_keys")]
                {
                    // TODO: sort the keys use thread-local buffer
                    let mut kvs: Vec<&(FastStr, OwnedLazyValue)> = vec.iter().collect();
                    kvs.sort_by(|(k1, _), (k2, _)| k1.as_str().cmp(k2.as_str()));
                    let mut map = serializer.serialize_map(Some(kvs.len()))?;
                    for (k, v) in kvs {
                        map.serialize_entry(k, v)?;
                    }
                    map.end()
                }

                #[cfg(not(feature = "sort_keys"))]
                {
                    let mut map = serializer.serialize_map(Some(vec.len()))?;
                    for (k, v) in vec {
                        map.serialize_entry(k, v)?;
                    }
                    map.end()
                }
            }
            LazyPacked::Parsed(Parsed::LazyArray(vec)) => vec.serialize(serializer),
            LazyPacked::Parsed(Parsed::String(s)) => s.serialize(serializer),
            LazyPacked::Parsed(Parsed::Number(n)) => n.serialize(serializer),
            LazyPacked::Parsed(Parsed::Bool(b)) => b.serialize(serializer),
            LazyPacked::Parsed(Parsed::Null) => serializer.serialize_none(),
        }
        // let raw = self.as_raw_str();
        // let mut s = serializer.serialize_struct(super::TOKEN, 1)?;
        // // will directly write raw in `LazyValueStrEmitter::seriazlie_str`
        // s.serialize_field(super::TOKEN, raw)?;
        // s.end()
    }
}

impl PartialEq for OwnedLazyValue {
    fn eq(&self, other: &Self) -> bool {
        todo!()
    }
}

impl Eq for OwnedLazyValue {}

#[derive(Debug, Clone, RefCast)]
#[repr(transparent)]
pub struct LazyObject(OwnedLazyValue);

impl std::ops::Deref for LazyObject {
    type Target = Vec<(FastStr, OwnedLazyValue)>;
    fn deref(&self) -> &Self::Target {
        if let LazyPacked::Parsed(Parsed::LazyObject(obj)) = &self.0 .0 {
            obj
        } else {
            unreachable!("must be a lazy object");
        }
    }
}

impl std::ops::DerefMut for LazyObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let LazyPacked::Parsed(Parsed::LazyObject(obj)) = &mut self.0 .0 {
            obj
        } else {
            unreachable!("must be a lazy object");
        }
    }
}

impl LazyObject {
    pub fn new() -> Self {
        Self(OwnedLazyValue(LazyPacked::Parsed(Parsed::LazyObject(
            Vec::new(),
        ))))
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self(OwnedLazyValue(LazyPacked::Parsed(Parsed::LazyObject(
            Vec::with_capacity(cap),
        ))))
    }

    pub fn append_pair(&mut self, key: FastStr, value: OwnedLazyValue) {
        if let LazyPacked::Parsed(Parsed::LazyObject(obj)) = &mut self.0 .0 {
            obj.push((key, value));
        } else {
            unreachable!("must be a lazy object");
        }
    }
}

impl From<Vec<(FastStr, OwnedLazyValue)>> for LazyObject {
    fn from(v: Vec<(FastStr, OwnedLazyValue)>) -> Self {
        Self(OwnedLazyValue(LazyPacked::Parsed(Parsed::LazyObject(v))))
    }
}

impl From<LazyObject> for OwnedLazyValue {
    fn from(v: LazyObject) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, RefCast)]
#[repr(transparent)]
pub struct LazyArray(OwnedLazyValue);

impl From<Vec<OwnedLazyValue>> for LazyArray {
    fn from(v: Vec<OwnedLazyValue>) -> Self {
        Self(OwnedLazyValue(LazyPacked::Parsed(Parsed::LazyArray(v))))
    }
}

impl From<LazyArray> for OwnedLazyValue {
    fn from(v: LazyArray) -> Self {
        v.0
    }
}

impl std::ops::DerefMut for LazyArray {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let LazyPacked::Parsed(Parsed::LazyArray(obj)) = &mut self.0 .0 {
            obj
        } else {
            unreachable!("must be a lazy array");
        }
    }
}

impl std::ops::Deref for LazyArray {
    type Target = Vec<OwnedLazyValue>;
    fn deref(&self) -> &Self::Target {
        if let LazyPacked::Parsed(Parsed::LazyArray(obj)) = &self.0 .0 {
            obj
        } else {
            unreachable!("must be a lazy array");
        }
    }
}

impl LazyArray {
    pub fn new() -> Self {
        Self(OwnedLazyValue(LazyPacked::Parsed(Parsed::LazyArray(
            Vec::new(),
        ))))
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self(OwnedLazyValue(LazyPacked::Parsed(Parsed::LazyArray(
            Vec::with_capacity(cap),
        ))))
    }
}

#[cfg(test)]
mod test {
    use crate::{pointer, to_lazyvalue, FastStr, OwnedLazyValue};

    #[test]
    fn test_owned_lazy_value() {
        let mut lv: OwnedLazyValue =
            crate::get_from_faststr(&FastStr::new(r#"{"a": "hello world"}"#), pointer![])
                .unwrap()
                .into();
        dbg!(&lv);

        if let Some(obj) = lv.as_object_mut() {
            for (k, v) in obj.iter_mut() {
                dbg!(k, v);
            }

            obj.append_pair(FastStr::new("foo"), to_lazyvalue("bar").unwrap());
        }

        dbg!(crate::to_string(&lv).unwrap());
    }
}
