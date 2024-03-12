use std::{
    borrow::Cow,
    fmt,
    fmt::{Debug, Display},
    hash::Hash,
    str::from_utf8_unchecked,
    sync::Arc,
};

use faststr::FastStr;

use crate::{
    from_str, get_unchecked, index::Index, input::JsonSlice, serde::Number, JsonType,
    JsonValueTrait, Result,
};

/// LazyValue wrappers a unparsed raw JSON text. It is borrowed from the origin JSON text.
///
/// LazyValue can be [`get`](crate::get),  [`get_unchecked`](crate::get_unchecked) or
/// [`deserialize`](crate::from_str) from a JSON text.
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
/// `LazyValue<'a>` can only be deserialized with borrowed.
/// If need to be owned, use [`OwnedLazyValue`](crate::OwnedLazyValue).
///
/// ```
/// # use sonic_rs::LazyValue;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Deserialize, Serialize, PartialEq)]
/// struct TestLazyValue<'a> {
///     #[serde(borrow)]
///     borrowed_lv: LazyValue<'a>,
/// }
///
/// let input = r#"{ "borrowed_lv": "hello"}"#;
///
/// let data: TestLazyValue = sonic_rs::from_str(input).unwrap();
/// assert_eq!(data.borrowed_lv.as_raw_str(), "\"hello\"");
/// ```
pub struct LazyValue<'a> {
    // the raw slice from origin json
    pub(crate) raw: JsonSlice<'a>,
    pub(crate) unescape: Option<Arc<str>>,
}

impl Default for LazyValue<'_> {
    fn default() -> Self {
        Self {
            raw: JsonSlice::Raw(&b"null"[..]),
            unescape: None,
        }
    }
}

impl<'a> Debug for LazyValue<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_tuple("LazyValue")
            .field(&format_args!("{}", &self.as_raw_str()))
            .finish()
    }
}

impl<'a> Display for LazyValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_raw_str())
    }
}

impl PartialEq for LazyValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw.as_ref() == other.raw.as_ref()
    }
}

impl<'a> Clone for LazyValue<'a> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            unescape: self.unescape.clone(),
        }
    }
}

impl PartialOrd for LazyValue<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for LazyValue<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.as_ref().cmp(other.raw.as_ref())
    }
}

impl<'a> Eq for LazyValue<'a> {}

impl<'a> Hash for LazyValue<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.as_ref().hash(state)
    }
}

impl<'a> JsonValueTrait for LazyValue<'a> {
    type ValueType<'v> = LazyValue<'v> where Self: 'v;

    fn as_bool(&self) -> Option<bool> {
        match self.raw.as_ref() {
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
        if !self.is_str() {
            None
        } else if let Some(escaped) = self.unescape.as_ref() {
            Some(escaped.as_ref())
        } else {
            // remove the quotes
            let origin = {
                let raw = self.as_raw_str().as_bytes();
                &raw[1..raw.len() - 1]
            };
            Some(unsafe { from_utf8_unchecked(origin) })
        }
    }

    fn get_type(&self) -> crate::JsonType {
        match self.raw.as_ref()[0] {
            b'-' | b'0'..=b'9' => JsonType::Number,
            b'"' => JsonType::String,
            b'{' => JsonType::Object,
            b'[' => JsonType::Array,
            b't' | b'f' => JsonType::Boolean,
            b'n' => JsonType::Null,
            _ => unreachable!(),
        }
    }

    fn get<I: Index>(&self, index: I) -> Option<LazyValue<'_>> {
        if let Some(key) = index.as_key() {
            self.get_key(key)
        } else if let Some(index) = index.as_index() {
            self.get_index(index)
        } else {
            unreachable!("index must be key or index")
        }
    }

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<LazyValue<'_>>
    where
        P::Item: Index,
    {
        let path = path.into_iter();
        match &self.raw {
            // #Safety
            // LazyValue is built with JSON validation, so we can use get_unchecked here.
            JsonSlice::Raw(r) => unsafe { get_unchecked(*r, path).ok() },
            JsonSlice::FastStr(f) => unsafe { get_unchecked(f, path).ok() },
        }
    }
}

impl<'a> LazyValue<'a> {
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

    /// Export the raw JSON text as `Cow<'de, str>`.  The lifetime `'de` is the origin JSON.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{get, LazyValue};
    ///
    /// let lv: LazyValue = sonic_rs::get(r#"{"a": "hello world"}"#, &["a"]).unwrap();
    /// assert_eq!(lv.as_raw_cow(), "\"hello world\"");
    /// ```
    pub fn as_raw_cow(&self) -> Cow<'a, str> {
        match &self.raw {
            JsonSlice::Raw(r) => Cow::Borrowed(unsafe { from_utf8_unchecked(r) }),
            JsonSlice::FastStr(f) => Cow::Owned(f.to_string()),
        }
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
        match &self.raw {
            JsonSlice::Raw(r) => unsafe { FastStr::from_u8_slice_unchecked(r) },
            JsonSlice::FastStr(f) => f.clone(),
        }
    }

    /// get with index from lazyvalue
    pub(crate) fn get_index(&'a self, index: usize) -> Option<Self> {
        let path = [index];
        match &self.raw {
            // #Safety
            // LazyValue is built with JSON validation, so we can use get_unchecked here.
            JsonSlice::Raw(r) => unsafe { get_unchecked(*r, path.iter()).ok() },
            JsonSlice::FastStr(f) => unsafe { get_unchecked(f, path.iter()).ok() },
        }
    }

    /// get with key from lazyvalue
    pub(crate) fn get_key(&'a self, key: &str) -> Option<Self> {
        let path = [key];
        match &self.raw {
            // #Safety
            // LazyValue is built with JSON validation, so we can use get_unchecked here.
            JsonSlice::Raw(r) => unsafe { get_unchecked(*r, path.iter()).ok() },
            JsonSlice::FastStr(f) => unsafe { get_unchecked(f, path.iter()).ok() },
        }
    }

    pub(crate) fn new(raw: JsonSlice<'a>, has_escaped: bool) -> Result<Self> {
        let unescape = if has_escaped {
            let unescape: Arc<str> = unsafe { crate::from_slice_unchecked(raw.as_ref()) }?;
            Some(unescape)
        } else {
            None
        };
        Ok(Self { raw, unescape })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pointer, to_array_iter};

    const TEST_JSON: &str = r#"{
        "bool": true,
        "int": -1,
        "uint": 0,
        "float": 1.1,
        "string": "hello",
        "string_escape": "\"hello\"",
        "array": [1,2,3],
        "object": {"a":"aaa"},
        "strempty": "",
        "objempty": {},
        "arrempty": [],
        "arrempty": []
    }"#;

    #[test]
    fn test_lazyvalue_export() {
        let f = FastStr::new(TEST_JSON);
        let value = unsafe { get_unchecked(&f, pointer![].iter()).unwrap() };
        assert_eq!(value.get("int").unwrap().as_raw_str(), "-1");
        assert_eq!(
            value.get("array").unwrap().as_raw_faststr().as_str(),
            "[1,2,3]"
        );
        assert_eq!(
            value
                .pointer(&pointer!["object", "a"])
                .unwrap()
                .as_raw_str()
                .as_bytes(),
            b"\"aaa\""
        );
        assert!(value.pointer(&pointer!["objempty", "a"]).is_none());
    }

    #[test]
    fn test_lazyvalue_is() {
        let value = unsafe { get_unchecked(TEST_JSON, pointer![].iter()).unwrap() };
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
    fn test_lazyvalue_get() {
        let value = unsafe { get_unchecked(TEST_JSON, pointer![].iter()).unwrap() };
        assert_eq!(value.get("int").as_i64().unwrap(), -1);
        assert_eq!(value.pointer(&pointer!["array", 2]).as_u64().unwrap(), 3);
        assert_eq!(
            value.pointer(&pointer!["object", "a"]).as_str().unwrap(),
            "aaa"
        );
        assert!(value.pointer(&pointer!["object", "b"]).is_none());
        assert!(value.pointer(&pointer!["object", "strempty"]).is_none());
        assert_eq!(value.pointer(&pointer!["objempty", "a"]).as_str(), None);
        assert!(value.pointer(&pointer!["arrempty", 1]).is_none());
        assert!(value.pointer(&pointer!["array", 3]).is_none());
        assert!(value.pointer(&pointer!["array", 4]).is_none());
        assert_eq!(value.pointer(&pointer!["arrempty", 1]).as_str(), None);
        assert_eq!(value.get("string").as_str().unwrap(), "hello");

        let value = unsafe { get_unchecked(TEST_JSON, pointer![].iter()).unwrap() };
        assert_eq!(value.get("string_escape").as_str().unwrap(), "\"hello\"");

        let value = unsafe { get_unchecked(TEST_JSON, pointer![].iter()).unwrap() };
        assert!(value.get("int").as_str().is_none());
        assert_eq!(value.get("int").as_i64(), Some(-1));
        assert_eq!(value.get("uint").as_i64(), Some(0));
        assert_eq!(value.get("float").as_f64(), Some(1.1));
    }

    #[test]
    fn test_lazyvalue_cow() {
        fn get_cow(json: &str) -> Option<Cow<'_, str>> {
            to_array_iter(json)
                .next()
                .map(|val| val.unwrap().as_raw_cow())
        }

        assert_eq!(get_cow("[true]").unwrap(), "true");
    }
}
