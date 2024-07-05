use faststr::FastStr;

use crate::{
    error::Result,
    input::{JsonInput, JsonSlice},
    lazyvalue::LazyValue,
    parser::{Parser, DEFAULT_KEY_BUF_CAPACITY},
    reader::{Read, Reader},
};

/// A lazied iterator for JSON object text. It will parse the JSON when iterating.
///
/// The item of the iterator is [`Result<LazyValue>`][`crate::LazyValue`].
///
/// # Examples
///```
/// use faststr::FastStr;
/// use sonic_rs::{to_object_iter, JsonValueTrait};
///
/// let json = FastStr::from(r#"{"a": null, "b":[1, 2, 3]}"#);
/// let iter = to_object_iter(&json);
///
/// for ret in iter {
///     // deal with errors
///     if ret.is_err() {
///         println!("{}", ret.unwrap_err());
///         return;
///     }
///     let (k, v) = ret.unwrap();
///     if k == "a" {
///         assert!(v.is_null());
///     } else if k == "b" {
///         assert_eq!(v.as_raw_str(), "[1, 2, 3]");
///     }
/// }
/// ```
pub struct ObjectJsonIter<'de>(ObjectInner<'de>);

/// A lazied iterator for JSON array text. It will parse the JSON when iterating.
///
/// The item of the iterator is [`Result<LazyValue>`][`crate::LazyValue`].
///
/// # Examples
/// ```
/// use sonic_rs::{to_array_iter, JsonValueTrait};
///
/// let iter = to_array_iter(r#"[0, 1, 2, 3, 4, 5, 6]"#);
/// for (i, ret) in iter.enumerate() {
///     let lv = ret.unwrap(); // get lazyvalue
///     assert_eq!(i.to_string(), lv.as_raw_str()); // lv is not parsed
///     assert_eq!(i, lv.as_u64().unwrap() as usize);
/// }
///
/// let iter = to_array_iter(r#"[1, 2, 3, 4, 5, 6"#);
/// for elem in iter {
///     // do something for each elem
///     // deal with errors when invalid json
///     if elem.is_err() {
///         assert!(elem
///             .unwrap_err()
///             .to_string()
///             .contains("Expected this character to be either a ',' or a ']'"));
///     }
/// }
/// ```
pub struct ArrayJsonIter<'de>(ArrayInner<'de>);

struct ObjectInner<'de> {
    json: JsonSlice<'de>,
    parser: Option<Parser<Read<'static>>>,
    strbuf: Vec<u8>,
    first: bool,
    ending: bool,
    check: bool,
}

struct ArrayInner<'de> {
    json: JsonSlice<'de>,
    parser: Option<Parser<Read<'static>>>,
    first: bool,
    ending: bool,
    check: bool,
}

impl<'de> ObjectInner<'de> {
    fn new(json: JsonSlice<'de>, check: bool) -> Self {
        Self {
            json,
            parser: None,
            strbuf: Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY),
            first: true,
            ending: false,
            check,
        }
    }

    fn next_entry_impl(&mut self, check: bool) -> Option<Result<(FastStr, LazyValue<'de>)>> {
        if self.ending {
            return None;
        }

        if self.parser.is_none() {
            let slice = self.json.as_ref();
            let slice = unsafe { std::slice::from_raw_parts(slice.as_ptr(), slice.len()) };
            let parser = Parser::new(Read::new(slice, check));
            // check invalid utf8
            match parser.read.check_utf8_final() {
                Err(err) if check => {
                    self.ending = true;
                    return Some(Err(err));
                }
                _ => {}
            }
            self.parser = Some(parser);
        }

        let parser = unsafe { self.parser.as_mut().unwrap_unchecked() };
        match parser.parse_entry_lazy(&mut self.strbuf, &mut self.first, check) {
            Ok(ret) => {
                if let Some((key, val, has_escaped)) = ret {
                    let val = self.json.slice_ref(val);
                    Some(LazyValue::new(val, has_escaped).map(|v| (key, v)))
                } else {
                    self.ending = true;
                    None
                }
            }
            Err(err) => {
                self.ending = true;
                Some(Err(err))
            }
        }
    }
}

impl<'de> ArrayInner<'de> {
    fn new(json: JsonSlice<'de>, check: bool) -> Self {
        Self {
            json,
            parser: None,
            first: true,
            ending: false,
            check,
        }
    }

    fn next_elem_impl(&mut self, check: bool) -> Option<Result<LazyValue<'de>>> {
        if self.ending {
            return None;
        }

        if self.parser.is_none() {
            let slice = self.json.as_ref();
            let slice = unsafe { std::slice::from_raw_parts(slice.as_ptr(), slice.len()) };
            let parser = Parser::new(Read::new(slice, check));
            // check invalid utf8
            match parser.read.check_utf8_final() {
                Err(err) if check => {
                    self.ending = true;
                    return Some(Err(err));
                }
                _ => {}
            }
            self.parser = Some(parser);
        }

        let parser = unsafe { self.parser.as_mut().unwrap_unchecked() };
        match parser.parse_array_elem_lazy(&mut self.first, check) {
            Ok(ret) => {
                if let Some((ret, has_escaped)) = ret {
                    let val = self.json.slice_ref(ret);
                    Some(LazyValue::new(val, has_escaped))
                } else {
                    self.ending = true;
                    None
                }
            }
            Err(err) => {
                self.ending = true;
                Some(Err(err))
            }
        }
    }
}

/// Traverse the JSON object text through a lazy iterator. The JSON parsing will doing when
/// iterating.
///
/// The item of the iterator is a key-value pair: ([FastStr][`faststr::FastStr`],
/// [Result<LazyValue>][`crate::LazyValue`]).
///
/// # Errors
///
/// If the JSON is empty, not a object or parse error, the result will be Err and the `next()` will
/// return `None`.
///
/// # Examples
///
/// ```
/// # use sonic_rs::to_object_iter;
/// use faststr::FastStr;
/// use sonic_rs::JsonValueTrait;
///
/// let json = FastStr::from(r#"{"a": null, "b":[1, 2, 3]}"#);
/// for ret in to_object_iter(&json) {
///     assert!(ret.is_ok());
///     let (k, v) = ret.unwrap();
///     if k == "a" {
///         assert!(v.is_null());
///     } else if k == "b" {
///         assert_eq!(v.as_raw_str(), "[1, 2, 3]");
///     }
/// }
///
/// // the JSON is invalid, will report error when encountering the error
/// for (i, ret) in to_object_iter(r#"{"a": null, "b":[1, 2, 3"#).enumerate() {
///     if i == 0 {
///         assert!(ret.is_ok());
///     }
///     if i == 1 {
///         assert!(ret.is_err());
///     }
/// }
/// ```
pub fn to_object_iter<'de, I: JsonInput<'de>>(json: I) -> ObjectJsonIter<'de> {
    ObjectJsonIter(ObjectInner::new(json.to_json_slice(), true))
}

/// Traverse the JSON array text through a lazy iterator. The JSON parsing will doing when
/// iterating.
///
/// The item of the iterator is [`Result<LazyValue>`][`crate::LazyValue`].
///
/// # Errors
///
/// If the JSON is empty, not array or parse error, it will return Err and `next()` will return
/// `None`.
///
/// # Examples
///
/// ```
/// # use sonic_rs::to_array_iter;
/// use sonic_rs::JsonValueTrait;
///
/// for (i, ret) in to_array_iter(r#"[0, 1, 2, 3, 4, 5, 6]"#).enumerate() {
///     let lv = ret.unwrap(); // get lazyvalue
///     assert_eq!(i.to_string(), lv.as_raw_str()); // lv is not parsed
///     assert_eq!(i, lv.as_u64().unwrap() as usize);
/// }
///
/// for elem in to_array_iter(r#"[1, 2, 3, 4, 5, 6"#) {
///     // do something for each elem
///     // deal with errors when invalid json
///     if elem.is_err() {
///         assert!(elem
///             .unwrap_err()
///             .to_string()
///             .contains("Expected this character to be either a ',' or a ']'"));
///     }
/// }
/// ```
pub fn to_array_iter<'de, I: JsonInput<'de>>(json: I) -> ArrayJsonIter<'de> {
    ArrayJsonIter(ArrayInner::new(json.to_json_slice(), true))
}

/// Traverse the JSON text through a lazy object iterator. The JSON parsing will doing when
/// iterating.
///
/// The item of the iterator is a key-value pair: ([FastStr][`faststr::FastStr`],
/// [Result<LazyValue>][`crate::LazyValue`]).
///
/// # Errors
///
/// If the JSON is empty, or not a object, the result will be Err and the `next()` will return
/// `None`.
///
/// # Safety
///
/// If the json is invalid, the result is undefined.
///
/// # Examples
///
/// ```
/// # use sonic_rs::to_object_iter_unchecked;
/// use faststr::FastStr;
/// use sonic_rs::JsonValueTrait;
///
/// let json = FastStr::from(r#"{"a": null, "b":[1, 2, 3]}"#);
/// for ret in unsafe { to_object_iter_unchecked(&json) } {
///     assert!(ret.is_ok());
///     let (k, v) = ret.unwrap();
///     if k == "a" {
///         assert!(v.is_null());
///     } else if k == "b" {
///         assert_eq!(v.as_raw_str(), "[1, 2, 3]");
///     }
/// }
/// ```
pub unsafe fn to_object_iter_unchecked<'de, I: JsonInput<'de>>(json: I) -> ObjectJsonIter<'de> {
    ObjectJsonIter(ObjectInner::new(json.to_json_slice(), false))
}

/// Traverse the JSON text through a lazy object iterator. The JSON parsing will doing when
/// iterating.
///
/// The item of the iterator is [`Result<LazyValue>`][`crate::LazyValue`].
///
/// # Errors
///
/// If the JSON is empty, or not a array, the result will be Err and the `next()` will return
/// `None`.
///
/// # Safety
///
/// If the json is invalid, the result is undefined.
///
/// # Examples
/// ```
/// # use sonic_rs::to_array_iter_unchecked;
/// use sonic_rs::JsonValueTrait;
///
/// for (i, ret) in unsafe { to_array_iter_unchecked(r#"[0, 1, 2, 3, 4, 5, 6]"#) }.enumerate() {
///     let lv = ret.unwrap(); // get lazyvalue
///     assert_eq!(i.to_string(), lv.as_raw_str()); // lv is not parsed
///     assert_eq!(i, lv.as_u64().unwrap() as usize);
/// }
///
/// // the JSON is empty
/// for elem in unsafe { to_array_iter_unchecked("") } {
///     assert!(elem.is_err());
/// }
/// ```
pub unsafe fn to_array_iter_unchecked<'de, I: JsonInput<'de>>(json: I) -> ArrayJsonIter<'de> {
    ArrayJsonIter(ArrayInner::new(json.to_json_slice(), false))
}

impl<'de> Iterator for ObjectJsonIter<'de> {
    type Item = Result<(FastStr, LazyValue<'de>)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_entry_impl(self.0.check)
    }
}

impl<'de> Iterator for ArrayJsonIter<'de> {
    type Item = Result<LazyValue<'de>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_elem_impl(self.0.check)
    }
}

#[cfg(test)]
mod test {
    use bytes::Bytes;

    use super::*;
    use crate::{value::JsonValueTrait, JsonType};

    #[test]
    fn test_object_iter() {
        let json = Bytes::from(
            r#"{
            "string": "Hello, world!",
            "number": 42,
            "boolean": true,
            "null": null,
            "array": ["foo","bar","baz"],
            "object": {"name": "Alice"},
            "empty": {},
            "": [],
            "escaped\"": "\"\"",
            "\t": "\n",
            "\u0000": "\u0001"
        }"#,
        );
        let _v: serde_json::Value = serde_json::from_slice(json.as_ref()).unwrap();
        let mut iter = to_object_iter(&json);
        let mut iter_unchecked = unsafe { to_object_iter_unchecked(&json) };

        let mut test_ok = |key: &str, val: &str, typ: JsonType| {
            let ret = iter.next().unwrap().unwrap();
            assert_eq!(ret.0.as_str(), key);
            assert_eq!(
                ret.1.as_raw_str().as_bytes(),
                val.as_bytes(),
                "key is {} ",
                key
            );
            assert_eq!(ret.1.get_type(), typ);

            let ret = iter_unchecked.next().unwrap().unwrap();
            assert_eq!(ret.0.as_str(), key);
            assert_eq!(
                ret.1.as_raw_str().as_bytes(),
                val.as_bytes(),
                "key is {} ",
                key
            );
            assert_eq!(ret.1.get_type(), typ);
        };
        test_ok("string", r#""Hello, world!""#, JsonType::String);
        test_ok("number", "42", JsonType::Number);
        test_ok("boolean", "true", JsonType::Boolean);
        test_ok("null", "null", JsonType::Null);
        test_ok("array", r#"["foo","bar","baz"]"#, JsonType::Array);
        test_ok("object", r#"{"name": "Alice"}"#, JsonType::Object);
        test_ok("empty", r#"{}"#, JsonType::Object);
        test_ok("", r#"[]"#, JsonType::Array);
        test_ok("escaped\"", r#""\"\"""#, JsonType::String);
        test_ok("\t", r#""\n""#, JsonType::String);
        test_ok("\x00", r#""\u0001""#, JsonType::String);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());

        let json = Bytes::from("{}");
        let mut iter = to_object_iter(&json);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());

        let json = Bytes::from("{xxxxxx");
        let mut iter = to_object_iter(&json);
        assert!(iter.next().unwrap().is_err());
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_array_iter() {
        let json = Bytes::from(
            r#"[
            "",
            "\\\"\"",
            "{\"a\":null}",
            "Hello, world!",
            0,
            1,
            11,
            1000,
            42,
            42.0,
            42e-1,
            4.2e+1,
            2333.2e+1,
            0.0000000999e8,
            true,
            null,
            ["foo","bar","baz"],
            {"name": "Alice"},
            [],
            {}
        ]"#,
        );
        let mut iter = to_array_iter(&json);
        let mut iter_unchecked = unsafe { to_array_iter_unchecked(&json) };
        let mut test_ok = |val: &str, typ: JsonType| {
            let ret: LazyValue<'_> = iter.next().unwrap().unwrap();
            assert_eq!(ret.as_raw_str(), val);
            assert_eq!(ret.get_type(), typ);

            let ret = iter_unchecked.next().unwrap().unwrap();
            assert_eq!(ret.as_raw_str().as_bytes(), val.as_bytes());
            assert_eq!(ret.get_type(), typ);
        };

        test_ok(r#""""#, JsonType::String);
        test_ok(r#""\\\"\"""#, JsonType::String);
        test_ok(r#""{\"a\":null}""#, JsonType::String);
        test_ok(r#""Hello, world!""#, JsonType::String);
        test_ok("0", JsonType::Number);
        test_ok("1", JsonType::Number);
        test_ok("11", JsonType::Number);
        test_ok("1000", JsonType::Number);
        test_ok("42", JsonType::Number);
        test_ok("42.0", JsonType::Number);
        test_ok("42e-1", JsonType::Number);
        test_ok("4.2e+1", JsonType::Number);
        test_ok("2333.2e+1", JsonType::Number);
        test_ok("0.0000000999e8", JsonType::Number);
        test_ok("true", JsonType::Boolean);
        test_ok("null", JsonType::Null);
        test_ok(r#"["foo","bar","baz"]"#, JsonType::Array);
        test_ok(r#"{"name": "Alice"}"#, JsonType::Object);
        test_ok(r#"[]"#, JsonType::Array);
        test_ok(r#"{}"#, JsonType::Object);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());

        let json = Bytes::from("[]");
        let mut iter = to_array_iter(&json);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());

        let json = Bytes::from("[xxxxxx");
        let mut iter = to_array_iter(&json);
        assert!(iter.next().unwrap().is_err());
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_iter_deserialize() {
        let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6]"#);
        let iter = to_array_iter(&json);
        let out: Vec<u8> = iter
            .flatten()
            .map(|e| crate::from_str::<u8>(e.as_raw_str()).unwrap_or_default())
            .collect();
        assert_eq!(out.as_slice(), &[1, 2, 3, 4, 5, 6]);

        let json = Bytes::from(r#"[1, true, "hello", null, 5, 6]"#);
        let iter = to_array_iter(&json);
        let out: Vec<JsonType> = iter.map(|e| e.get_type()).collect();
        println!("array elem type is {:?}", out);
    }

    #[test]
    fn test_num_iter() {
        for i in to_array_iter("[6,-9E6]") {
            println!("{:?}", i.unwrap().as_raw_str());
        }
    }

    #[test]
    fn test_json_iter_for_utf8() {
        let data = [b'[', b'"', 0, 0, 0, 0x80, 0x90, b'"', b']'];
        let iter = to_array_iter(&data[..]);
        for item in iter {
            assert_eq!(
                item.err().unwrap().to_string(),
                "Invalid UTF-8 characters in json at line 1 column \
                 5\n\n\t[\"\0\0\0��\"]\n\t.....^...\n"
            );
        }

        let data = [
            b'{', b'"', 0, 0, 0, 0x80, 0x90, b'"', b':', b'"', b'"', b'}',
        ];
        let iter = to_object_iter(&data[..]);
        for item in iter {
            assert_eq!(
                item.err().unwrap().to_string(),
                "Invalid UTF-8 characters in json at line 1 column \
                 5\n\n\t{\"\0\0\0��\":\"\"}\n\t.....^......\n"
            );
        }
    }
}
