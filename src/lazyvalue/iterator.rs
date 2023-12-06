use crate::error::Result;
use crate::input::JsonInput;
use crate::input::JsonSlice;
use crate::lazyvalue::LazyValue;
use crate::parser::{Parser, DEFAULT_KEY_BUF_CAPACITY};
use crate::reader::SliceRead;
use faststr::FastStr;

/// A lazied iterator for JSON object.
pub struct ObjectIntoIter<'de>(ObjectInner<'de>);

/// A lazied iterator for JSON array.
pub struct ArrayIntoIter<'de>(ArrayInner<'de>);

struct ObjectInner<'de> {
    json: JsonSlice<'de>,
    parser: Option<Parser<SliceRead<'static>>>,
    strbuf: Vec<u8>,
    first: bool,
    ending: bool,
}

struct ArrayInner<'de> {
    json: JsonSlice<'de>,
    parser: Option<Parser<SliceRead<'static>>>,
    first: bool,
    ending: bool,
}

/// A lazied iterator for JSON object.
/// # Safety
/// If the json is invalid, the result is undefined.
pub struct UnsafeObjectIntoIter<'de>(ObjectInner<'de>);

/// A lazied iterator for JSON array.
/// # Safety
/// If the json is invalid, the result is undefined.
pub struct UnsafeArrayIntoIter<'de>(ArrayInner<'de>);

impl<'de> ObjectInner<'de> {
    fn new(json: JsonSlice<'de>) -> Self {
        Self {
            json,
            parser: None,
            strbuf: Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY),
            first: true,
            ending: false,
        }
    }

    fn next_entry_impl(&mut self, check: bool) -> Option<Result<(FastStr, LazyValue<'de>)>> {
        if self.ending {
            return None;
        }

        if self.parser.is_none() {
            let slice = self.json.as_ref();
            let slice = unsafe { std::slice::from_raw_parts(slice.as_ptr(), slice.len()) };
            let parser = Parser::new(SliceRead::new(slice));
            self.parser = Some(parser);
        }

        let parser = unsafe { self.parser.as_mut().unwrap_unchecked() };
        match parser.parse_entry_lazy(&mut self.strbuf, &mut self.first, check) {
            Ok(ret) => {
                if let Some((key, val)) = ret {
                    let val = self.json.slice_ref(val);
                    Some(Ok((key, LazyValue::new(val))))
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
    fn new(json: JsonSlice<'de>) -> Self {
        Self {
            json,
            parser: None,
            first: true,
            ending: false,
        }
    }

    fn next_elem_impl(&mut self, check: bool) -> Option<Result<LazyValue<'de>>> {
        if self.ending {
            return None;
        }

        if self.parser.is_none() {
            let slice = self.json.as_ref();
            let slice = unsafe { std::slice::from_raw_parts(slice.as_ptr(), slice.len()) };
            let parser = Parser::new(SliceRead::new(slice));
            self.parser = Some(parser);
        }

        let parser = unsafe { self.parser.as_mut().unwrap_unchecked() };
        match parser.parse_array_elem_lazy(&mut self.first, check) {
            Ok(ret) => {
                if let Some(ret) = ret {
                    let val = self.json.slice_ref(ret);
                    Some(Ok(LazyValue::new(val)))
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

/// Convert a json to a lazy ObjectIntoIter. The iterator is lazied and the parsing will doing when iterating.
/// The item of the iterator is a Result. If parse error, it will return Err.
pub fn to_object_iter<'de, I: JsonInput<'de>>(json: I) -> ObjectIntoIter<'de> {
    ObjectIntoIter(ObjectInner::new(json.to_json_slice()))
}

/// Convert a json to a lazy ArrayIntoIter. The iterator is lazied and the parsing will doing when iterating.
/// The item of the iterator is a Result. If parse error, it will return Err.
pub fn to_array_iter<'de, I: JsonInput<'de>>(json: I) -> ArrayIntoIter<'de> {
    ArrayIntoIter(ArrayInner::new(json.to_json_slice()))
}

/// Convert a json to a lazy ObjectIntoIter. The iterator is lazied and the parsing will doing when iterating.
/// The item of the iterator is a Result. If parse error, it will return Err.
/// # Safety
/// If the json is invalid, the result is undefined.
pub unsafe fn to_object_iter_unchecked<'de, I: JsonInput<'de>>(
    json: I,
) -> UnsafeObjectIntoIter<'de> {
    UnsafeObjectIntoIter(ObjectInner::new(json.to_json_slice()))
}

/// Convert a json to a lazy ArrayIntoIter. The iterator is lazied and the parsing will doing when iterating.
/// The item of the iterator is a Result. If parse error, it will return Err.
/// # Safety
/// If the json is invalid, the result is undefined.
pub unsafe fn to_array_iter_unchecked<'de, I: JsonInput<'de>>(json: I) -> UnsafeArrayIntoIter<'de> {
    UnsafeArrayIntoIter(ArrayInner::new(json.to_json_slice()))
}

impl<'de> Iterator for ObjectIntoIter<'de> {
    type Item = Result<(FastStr, LazyValue<'de>)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_entry_impl(true)
    }
}

impl<'de> Iterator for ArrayIntoIter<'de> {
    type Item = Result<LazyValue<'de>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_elem_impl(true)
    }
}

impl<'de> Iterator for UnsafeObjectIntoIter<'de> {
    type Item = Result<(FastStr, LazyValue<'de>)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_entry_impl(false)
    }
}

impl<'de> Iterator for UnsafeArrayIntoIter<'de> {
    type Item = Result<LazyValue<'de>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_elem_impl(false)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{value::JsonValueTrait, JsonType};
    use bytes::Bytes;

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
            assert_eq!(ret.1.as_raw_slice(), val.as_bytes(), "key is {} ", key);
            assert_eq!(ret.1.get_type(), typ);

            let ret = iter_unchecked.next().unwrap().unwrap();
            assert_eq!(ret.0.as_str(), key);
            assert_eq!(ret.1.as_raw_slice(), val.as_bytes(), "key is {} ", key);
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
            assert_eq!(ret.as_raw_slice(), val.as_bytes());
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
            .map(|e| e.deserialize::<u8>().unwrap_or_default())
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
}
