use std::{
    fmt,
    fmt::{Debug, Display},
    hash::Hash,
    str::from_utf8_unchecked,
    sync::atomic::AtomicPtr,
};

use faststr::FastStr;

use super::value::HasEsc;
use crate::{
    from_str, get_unchecked,
    index::Index,
    input::{self, JsonSlice},
    lazyvalue::{
        iterator::{OwnedArrayJsonIter, OwnedObjectJsonIter},
        value::Inner,
    },
    parser::Parser,
    serde::Number,
    ArrayJsonIter, JsonInput, JsonType, JsonValueTrait, LazyValue, ObjectJsonIter, Result,
};

/// OwnedLazyValue wrappers a unparsed raw JSON text. It is owned.
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
#[derive(Clone)]
pub struct OwnedLazyValue {
    // the raw slice from origin json
    pub(crate) raw: FastStr,
    pub(crate) inner: Inner,
}

impl JsonValueTrait for OwnedLazyValue {
    type ValueType<'v> = OwnedLazyValue;

    fn as_bool(&self) -> Option<bool> {
        match self.raw.as_bytes() {
            b"true" => Some(true),
            b"false" => Some(false),
            _ => None,
        }
    }

    fn as_number(&self) -> Option<Number> {
        if let Ok(num) = from_str(self.as_raw_str()) {
            Some(num)
        } else {
            None
        }
    }

    fn as_raw_number(&self) -> Option<crate::RawNumber> {
        if let Ok(num) = from_str(self.as_raw_str()) {
            Some(num)
        } else {
            None
        }
    }

    fn as_str(&self) -> Option<&str> {
        if !self.is_str() {
            return None;
        }

        if self.inner.no_escaped() {
            // remove the quotes
            let origin = {
                let raw = self.as_raw_str().as_bytes();
                &raw[1..raw.len() - 1]
            };
            Some(unsafe { from_utf8_unchecked(origin) })
        } else {
            self.inner.parse_from(self.raw.as_ref())
        }
    }

    fn get_type(&self) -> crate::JsonType {
        match self.raw.as_bytes()[0] {
            b'-' | b'0'..=b'9' => JsonType::Number,
            b'"' => JsonType::String,
            b'{' => JsonType::Object,
            b'[' => JsonType::Array,
            b't' | b'f' => JsonType::Boolean,
            b'n' => JsonType::Null,
            _ => unreachable!(),
        }
    }

    fn get<I: Index>(&self, index: I) -> Option<OwnedLazyValue> {
        if let Some(key) = index.as_key() {
            self.get_key(key)
        } else if let Some(index) = index.as_index() {
            self.get_index(index)
        } else {
            unreachable!("index must be key or index")
        }
    }

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<OwnedLazyValue>
    where
        P::Item: Index,
    {
        let lv = unsafe { get_unchecked(&self.raw, path).ok() };
        lv.map(|v| v.into())
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
        unsafe { from_utf8_unchecked(self.raw.as_ref()) }
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
        self.raw.clone()
    }

    pub fn into_object_iter(mut self) -> Option<OwnedObjectJsonIter> {
        if self.is_object() {
            Some(OwnedObjectJsonIter(ObjectJsonIter::new_inner(
                std::mem::take(&mut self.raw).into(),
            )))
        } else {
            None
        }
    }

    pub fn into_array_iter(mut self) -> Option<OwnedArrayJsonIter> {
        if self.is_array() {
            Some(OwnedArrayJsonIter(ArrayJsonIter::new_inner(
                std::mem::take(&mut self.raw).into(),
            )))
        } else {
            None
        }
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

    /// get with index from lazyvalue
    pub(crate) fn get_index(&self, index: usize) -> Option<Self> {
        let path = [index];
        let lv = unsafe { get_unchecked(&self.raw, path.iter()).ok() };
        lv.map(|v| v.into())
    }

    /// get with key from lazyvalue
    pub(crate) fn get_key(&self, key: &str) -> Option<Self> {
        let path = [key];
        let lv = unsafe { get_unchecked(&self.raw, path.iter()).ok() };
        lv.map(|v| v.into())
    }

    pub(crate) fn new(raw: JsonSlice, status: HasEsc) -> Self {
        let raw = match raw {
            JsonSlice::Raw(r) => FastStr::new(unsafe { from_utf8_unchecked(r) }),
            JsonSlice::FastStr(f) => f.clone(),
        };

        Self {
            raw,
            inner: Inner {
                status,
                unescaped: AtomicPtr::new(std::ptr::null_mut()),
            },
        }
    }
}

impl<'de> From<LazyValue<'de>> for OwnedLazyValue {
    fn from(lv: LazyValue<'de>) -> Self {
        let raw = match lv.raw {
            JsonSlice::Raw(r) => FastStr::new(unsafe { from_utf8_unchecked(r) }),
            JsonSlice::FastStr(f) => f.clone(),
        };
        Self {
            raw,
            inner: lv.inner,
        }
    }
}

impl Debug for OwnedLazyValue {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_tuple("OwnedLazyValue")
            .field(&format_args!("{}", &self.as_raw_str()))
            .finish()
    }
}

impl Display for OwnedLazyValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_raw_str())
    }
}

impl Default for OwnedLazyValue {
    fn default() -> Self {
        Self {
            raw: FastStr::new("null"),
            inner: Inner::default(),
        }
    }
}

impl PartialEq for OwnedLazyValue {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl PartialOrd for OwnedLazyValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OwnedLazyValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl Eq for OwnedLazyValue {}

impl Hash for OwnedLazyValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}
