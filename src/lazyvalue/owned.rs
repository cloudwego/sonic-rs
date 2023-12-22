use std::{cell::UnsafeCell, hash::Hash, str::from_utf8_unchecked};

use faststr::FastStr;

use crate::{
    from_str, get_unchecked,
    index::Index,
    input::JsonSlice,
    parser::Parser,
    reader::{Reader, Reference, SliceRead},
    serde::Number,
    JsonType, JsonValueTrait, LazyValue,
};

/// LazyValue is a value that wrappers a raw JSON text. It is used for lazy parsing, which means the
/// JSON text is not parsed until it is used.
///
/// # Examples
///
/// ```
/// use sonic_rs::{get, JsonValueTrait, LazyValue, Value};
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
/// let lv_a: LazyValue = get(input, &["a"]).unwrap();
/// let lv_c: LazyValue = get(input, &["c"]).unwrap();
///
/// // use as_raw_xx to get the unparsed JSON text
/// assert_eq!(lv_a.as_raw_str(), "\"hello world\"");
/// assert_eq!(lv_c.as_raw_str(), "[0, 1, 2]");
///
/// // use as_xx to get the parsed value
/// assert_eq!(lv_a.as_str().unwrap(), "hello world");
/// assert_eq!(lv_c.as_str(), None);
/// assert!(lv_c.is_array());
/// ```
///
/// # Serde Examples
///
/// `OwnedLazyValue` can be deserialized with borrowed or owned.
/// If borrowed, lifetime `'a` is the origin JSON text.
/// If owned, lifetime `'a` is not associate with origin JSON text.
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
#[derive(Debug)]
pub struct OwnedLazyValue {
    // the raw slice from origin json
    pub(crate) raw: FastStr,
    // used for deserialize escaped strings
    own: UnsafeCell<Vec<u8>>,
}

impl PartialEq for OwnedLazyValue {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Clone for OwnedLazyValue {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            own: UnsafeCell::new(Vec::new()),
        }
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

    fn as_str(&self) -> Option<&str> {
        let mut parser = Parser::new(SliceRead::new(self.as_raw_str().as_bytes()));
        parser.read.eat(1);
        match parser.parse_string_raw(unsafe { &mut *self.own.get() }) {
            Ok(Reference::Borrowed(u)) => unsafe { Some(from_utf8_unchecked(u)) },
            Ok(Reference::Copied(u)) => unsafe { Some(from_utf8_unchecked(u)) },
            _ => None,
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

    pub(crate) fn new(raw: JsonSlice) -> Self {
        let raw = match raw {
            JsonSlice::Raw(r) => FastStr::new(unsafe { from_utf8_unchecked(r) }),
            JsonSlice::FastStr(f) => f.clone(),
        };
        Self {
            raw,
            own: UnsafeCell::new(Vec::new()),
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
            own: UnsafeCell::new(Vec::new()),
        }
    }
}
