use crate::error::Result;
use crate::input::JsonInput;
use crate::input::JsonSlice;
use crate::lazyvalue::LazyValue;
use crate::parser::{Parser, DEFAULT_KEY_BUF_CAPACITY};
use crate::reader::SliceRead;
use faststr::FastStr;

/// A lazied iterator for JSON object.
/// ObjectIterator can be used as `into_iter` directly.
pub struct ObjectIntoIter<'de> {
    json: JsonSlice<'de>,
    parser: Option<Parser<SliceRead<'static>>>,
    strbuf: Vec<u8>,
    first: bool,
    ending: bool,
}

/// A lazied iterator for JSON array.
/// ArrayIterator can be used as `into_iter` directly.
pub struct ArrayIntoIter<'de> {
    json: JsonSlice<'de>,
    parser: Option<Parser<SliceRead<'static>>>,
    first: bool,
    ending: bool,
}

impl<'de> ObjectIntoIter<'de> {
    fn new(json: JsonSlice<'de>) -> Self {
        Self {
            json,
            parser: None,
            strbuf: Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY),
            first: true,
            ending: false,
        }
    }

    fn next_entry_impl(&mut self) -> Option<Result<(FastStr, LazyValue<'de>)>> {
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
        match parser.parse_entry_lazy(&mut self.strbuf, &mut self.first) {
            Ok(ret) => {
                if let Some(ret) = ret {
                    let key = ret.0;
                    let val = self.json.slice_ref(ret.1);
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

impl<'de> ArrayIntoIter<'de> {
    fn new(json: JsonSlice<'de>) -> Self {
        Self {
            json,
            parser: None,
            first: true,
            ending: false,
        }
    }

    fn next_elem_impl(&mut self) -> Option<Result<LazyValue<'de>>> {
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
        match parser.parse_array_elem_lazy(&mut self.first) {
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
    ObjectIntoIter::new(json.to_json_slice())
}

/// Convert a json to a lazy ArrayIntoIter. The iterator is lazied and the parsing will doing when iterating.
/// The item of the iterator is a Result. If parse error, it will return Err.
pub fn to_array_iter<'de, I: JsonInput<'de>>(json: I) -> ArrayIntoIter<'de> {
    ArrayIntoIter::new(json.to_json_slice())
}

impl<'de> Iterator for ObjectIntoIter<'de> {
    type Item = Result<(FastStr, LazyValue<'de>)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_entry_impl()
    }
}

impl<'de> Iterator for ArrayIntoIter<'de> {
    type Item = Result<LazyValue<'de>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_elem_impl()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{value::JsonValue, JsonType};
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
            "escaped\"": "\"\""
        }"#,
        );
        let mut iter = to_object_iter(&json);

        let mut test_ok = |key: &str, val: &str, typ: JsonType| {
            let ret = iter.next().unwrap().unwrap();
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
            "Hello, world!",
            42,
            true,
            null,
            ["foo","bar","baz"],
            {"name": "Alice"},
            [],
            {}
        ]"#,
        );
        let mut iter = to_array_iter(&json);
        let mut test_ok = |val: &str, typ: JsonType| {
            let ret = iter.next().unwrap().unwrap();
            assert_eq!(ret.as_raw_slice(), val.as_bytes());
            assert_eq!(ret.get_type(), typ);
        };

        test_ok(r#""Hello, world!""#, JsonType::String);
        test_ok("42", JsonType::Number);
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
}
