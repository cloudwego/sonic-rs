use std::borrow::Cow;
use std::cell::UnsafeCell;
use std::str::from_utf8_unchecked;

use bytes::Bytes;
use faststr::FastStr;
use serde::Deserialize;

use crate::error::Result;
use crate::get_unchecked;
use crate::index::Index;
use crate::input::JsonSlice;
use crate::parser::Parser;
use crate::reader::Reader;
use crate::reader::Reference;
use crate::reader::SliceRead;
use crate::serde::Deserializer;
use crate::serde::Number;
use crate::JsonType;
use crate::{from_str, JsonValueTrait};

/// LazyValue is a value that wrappers the raw JSON text. It is similar as `RawValue` but with
/// more APIs. It is used for lazy parsing, which means the JSON text is not parsed until it is
/// used.
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
///
/// // if we want parse LazyValue into `Value`, just use try_from
/// let mut value = Value::try_from(lv_c).unwrap();
/// value[0] = 1.into();
/// assert_eq!(value, [1, 1, 2]);
///
/// // also, we can parse LazyValue into Rust types
/// let lv_d: LazyValue = get(input, &["d", "sonic"]).unwrap();
/// let mut v: String = sonic_rs::from_str(lv_d.as_raw_str()).unwrap();
/// assert_eq!(v, "rs");
/// ```
#[derive(Debug)]
pub struct LazyValue<'de> {
    // the raw slice from origin json
    raw: JsonSlice<'de>,
    // used for deserialize escaped strings
    own: UnsafeCell<Vec<u8>>,
}

impl<'de> JsonValueTrait for LazyValue<'de> {
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
        let mut parser = Parser::new(SliceRead::new(self.as_raw_slice()));
        parser.read.eat(1);
        match parser.parse_string_raw(unsafe { &mut *self.own.get() }) {
            Ok(Reference::Borrowed(u)) => unsafe { Some(from_utf8_unchecked(u)) },
            Ok(Reference::Copied(u)) => unsafe { Some(from_utf8_unchecked(u)) },
            _ => None,
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

    fn get<I: Index>(&self, index: I) -> Option<Self::ValueType<'_>> {
        index.lazyvalue_index_into(self)
    }

    fn pointer<P: IntoIterator>(&self, path: P) -> Option<Self::ValueType<'_>>
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

impl<'de> LazyValue<'de> {
    /// Deserialize the raw json text into Rust type
    pub fn deserialize<T: Deserialize<'de>>(&'de self) -> Result<T> {
        let reader = SliceRead::new(self.raw.as_ref());
        let mut deserializer = Deserializer::new(reader);
        T::deserialize(&mut deserializer)
    }

    /// export the raw json text as slice
    pub fn as_raw_slice(&self) -> &[u8] {
        self.raw.as_ref()
    }

    /// export the raw json text as str
    pub fn as_raw_str(&self) -> &str {
        // # Safety
        // it is validate when using to_object_iter/get ...
        // if use `get_unchecked` unsafe apis, it must ensured by the user at first
        unsafe { from_utf8_unchecked(self.raw.as_ref()) }
    }

    /// export the raw json text as Cow str, the lifetime of Cow is the same as the json text
    pub fn as_raw_cow(&self) -> Cow<'de, str> {
        match &self.raw {
            JsonSlice::Raw(r) => Cow::Borrowed(unsafe { from_utf8_unchecked(r) }),
            JsonSlice::FastStr(f) => Cow::Owned(f.to_string()),
        }
    }

    /// export the raw json text as bytes
    /// Note: if the input json is not bytes or faststr, there will be a copy.
    pub fn as_raw_bytes(&self) -> Bytes {
        match &self.raw {
            JsonSlice::Raw(r) => Bytes::copy_from_slice(r),
            // if build from Bytes,
            JsonSlice::FastStr(f) => f.clone().into_bytes(),
        }
    }

    /// export the raw json text as faststr
    /// Note: if the input json is not bytes or faststr, there will be a copy.
    pub fn as_raw_faststr(&self) -> FastStr {
        match &self.raw {
            JsonSlice::Raw(r) => unsafe { FastStr::from_u8_slice_unchecked(r) },
            JsonSlice::FastStr(f) => f.clone(),
        }
    }

    /// get with index from lazyvalue
    pub fn get_index(&'de self, index: usize) -> Option<Self> {
        let path = [index];
        match &self.raw {
            // #Safety
            // LazyValue is built with JSON validation, so we can use get_unchecked here.
            JsonSlice::Raw(r) => unsafe { get_unchecked(*r, path.iter()).ok() },
            JsonSlice::FastStr(f) => unsafe { get_unchecked(f, path.iter()).ok() },
        }
    }

    /// get with key from lazyvalue
    pub fn get_key(&'de self, key: &str) -> Option<Self> {
        let path = [key];
        match &self.raw {
            // #Safety
            // LazyValue is built with JSON validation, so we can use get_unchecked here.
            JsonSlice::Raw(r) => unsafe { get_unchecked(*r, path.iter()).ok() },
            JsonSlice::FastStr(f) => unsafe { get_unchecked(f, path.iter()).ok() },
        }
    }

    pub fn into_cow_str(self) -> Result<Cow<'de, str>> {
        let mut parser = Parser::new(SliceRead::new(self.raw.as_ref()));
        parser.read.eat(1);
        match parser.parse_string_raw(unsafe { &mut *self.own.get() })? {
            Reference::Borrowed(u) => match self.raw {
                JsonSlice::Raw(raw) => unsafe {
                    // u must be from raw
                    let len = u.len();
                    let ptr = u.as_ptr();
                    let offset = ptr.offset_from(raw.as_ptr()) as usize;
                    debug_assert!(offset + len <= raw.len());
                    Ok(Cow::Borrowed(from_utf8_unchecked(
                        raw.get_unchecked(offset..offset + len),
                    )))
                },
                JsonSlice::FastStr(_) => {
                    Ok(Cow::Owned(String::from(unsafe { from_utf8_unchecked(u) })))
                }
            },
            Reference::Copied(_) => unsafe {
                Ok(Cow::Owned(String::from_utf8_unchecked(
                    self.own.into_inner(),
                )))
            },
        }
    }

    pub(crate) fn new(raw: JsonSlice<'de>) -> Self {
        Self {
            raw,
            own: UnsafeCell::new(Vec::new()),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::to_array_iter;
    use crate::value::JsonValueTrait;
    use crate::{get_unchecked, pointer};

    use super::*;

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
                .as_raw_bytes()
                .as_ref(),
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
        assert_eq!(
            value.get("string").unwrap().into_cow_str().unwrap(),
            "hello"
        );

        let value = unsafe { get_unchecked(TEST_JSON, pointer![].iter()).unwrap() };
        assert_eq!(
            value.get("string_escape").unwrap().into_cow_str().unwrap(),
            "\"hello\""
        );

        let value = unsafe { get_unchecked(TEST_JSON, pointer![].iter()).unwrap() };
        assert!(value.get("int").unwrap().into_cow_str().is_err());
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
