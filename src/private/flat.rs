//! Not Public API. A simple flatten DOM from JSON.

use std::{borrow::Cow, str::from_utf8_unchecked};

use super::config::Config;
use crate::{parser::Parser, reader::PaddedSliceRead, value::visitor::JsonVisitor, Result};

pub fn dom_from_slice_config(json: &[u8], config: Config) -> Result<Document> {
    use crate::util::utf8::from_utf8_lossy;

    let cow = if config.validate_string {
        from_utf8_lossy(json)
    } else {
        Cow::Borrowed(unsafe { from_utf8_unchecked(json) })
    };

    let json = cow.as_bytes();
    let mut doc = Document {
        has_utf8_lossy: matches!(cow, std::borrow::Cow::Owned(_)),
        ..Default::default()
    };
    doc.parse_bytes_impl(json, config)?;
    Ok(doc)
}

pub fn dom_from_slice(json: &[u8]) -> Result<Document> {
    use crate::util::utf8::from_utf8;
    let json = {
        let json = from_utf8(json)?;
        json.as_bytes()
    };

    let mut doc = Document::default();
    doc.parse_bytes_impl(json, Config::default())?;
    Ok(doc)
}

pub unsafe fn dom_from_slice_unchecked(json: &[u8]) -> Result<Document> {
    let mut doc = Document::default();
    doc.parse_bytes_impl(json, Config::default())?;
    Ok(doc)
}

// JSON Value Type
const NULL: u64 = 0;
const BOOL: u64 = 2;
const FALSE: u64 = BOOL; // 2
const TRUE: u64 = (1 << 3) | BOOL; // 10
const NUMBER: u64 = 3;
const UINT: u64 = NUMBER; // 3
const SINT: u64 = (1 << 3) | NUMBER; // 11
const REAL: u64 = (2 << 3) | NUMBER; // 19
const RAWNUMBER: u64 = (3 << 3) | NUMBER; // 27
const STRING: u64 = 4;
const STRING_COMMON: u64 = STRING; // 4
const STRING_HASESCAPED: u64 = (1 << 3) | STRING; // 12
const OBJECT: u64 = 6;
const ARRAY: u64 = 7;

/// JSON Type Mask
const POS_MASK: u64 = (!0) << 32;
const POS_BITS: u64 = 32;
const TYPE_MASK: u64 = 0xFF;
const TYPE_BITS: u64 = 8;
const CON_LEN_MASK: u64 = (!0u64) >> 32;
const CON_LEN_BITS: u64 = 32;

#[repr(C)]
#[derive(Debug, Default)]
pub struct Value {
    /// (low)
    /// | type (8 bits) | reserved (24 bits) | pos (32 bits)     |
    /// | type (8 bits) |                    | str len (32 bits) |
    typ: u64,
    ///
    /// |   number val, rawnumber ptr, string ptr, object/array ptr (64 bits)   |
    /// |   next offset (32 bits) | object/array len (32 bits)   |
    val: u64,
}

impl From<bool> for Value {
    #[inline(always)]
    fn from(val: bool) -> Self {
        Self {
            typ: if val { TRUE } else { FALSE },
            val: 0,
        }
    }
}

impl From<()> for Value {
    #[inline(always)]
    fn from(_val: ()) -> Self {
        Self { typ: NULL, val: 0 }
    }
}

impl Value {
    pub fn new_bool(val: bool, pos: usize) -> Self {
        let typ = if val { TRUE } else { FALSE };
        Self {
            typ: typ | ((pos as u64) << POS_BITS),
            val: val as u64,
        }
    }

    pub fn new_null(pos: usize) -> Self {
        Self {
            typ: NULL | ((pos as u64) << POS_BITS),
            val: 0,
        }
    }

    pub fn new_i64(val: i64, pos: usize) -> Self {
        Self {
            typ: SINT | ((pos as u64) << POS_BITS),
            val: val as u64,
        }
    }

    pub fn new_u64(val: u64, pos: usize) -> Self {
        Self {
            typ: UINT | ((pos as u64) << POS_BITS),
            val,
        }
    }

    pub fn new_number(val: &str) -> Self {
        Self {
            typ: RAWNUMBER | ((val.len() as u64) << POS_BITS),
            val: val.as_ptr() as u64,
        }
    }

    pub unsafe fn new_f64_unchecked(val: f64, pos: usize) -> Self {
        Self {
            typ: REAL | ((pos as u64) << POS_BITS),
            val: val.to_bits(),
        }
    }

    pub fn new_str(val: &str, has_escaped: bool) -> Self {
        let t = if !has_escaped {
            STRING_COMMON
        } else {
            STRING_HASESCAPED
        };
        Self {
            typ: t | ((val.len() as u64) << POS_BITS),
            val: val.as_ptr() as u64,
        }
    }

    pub fn new_array(pos: usize) -> Self {
        Self {
            typ: ARRAY | ((pos as u64) << POS_BITS),
            val: 0,
        }
    }

    pub fn new_object(pos: usize) -> Self {
        Self {
            typ: OBJECT | ((pos as u64) << POS_BITS),
            val: 0,
        }
    }

    fn str(&self) -> &str {
        unsafe {
            let ptr = self.val as *const u8;
            let len = (self.typ >> POS_BITS) as usize;
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len))
        }
    }

    fn typ(&self) -> u64 {
        self.typ & TYPE_MASK
    }

    fn next(&self) -> &Self {
        if self.typ() == OBJECT || self.typ() == ARRAY {
            let offset = self.val >> CON_LEN_BITS;
            unsafe { &*(self as *const Value).offset(offset as isize) }
        } else {
            unsafe { &*(self as *const Value).offset(1) }
        }
    }

    fn children(&self) -> &Self {
        if self.typ() == OBJECT || self.typ() == ARRAY {
            unsafe { &*(self as *const Value).offset(1) }
        } else {
            unreachable!("not a container")
        }
    }

    fn number(&self) -> &str {
        unsafe {
            let ptr = self.val as *const u8;
            let len = (self.typ >> POS_BITS) as usize;
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len))
        }
    }

    fn len(&self) -> usize {
        (self.val & CON_LEN_MASK) as usize
    }

    fn serialize<W: crate::writer::WriteExt, F: crate::format::Formatter>(
        &self,
        w: &mut W,
        f: &mut F,
    ) -> std::io::Result<()> {
        let typ = self.typ & TYPE_MASK;
        match typ {
            NULL => f.write_null(w)?,
            FALSE => f.write_bool(w, false)?,
            TRUE => f.write_bool(w, true)?,
            UINT => f.write_u64(w, self.val)?,
            SINT => f.write_i64(w, self.val as i64)?,
            REAL => f.write_f64(w, f64::from_bits(self.val))?,
            RAWNUMBER => f.write_raw_value(w, self.number())?,
            STRING_COMMON | STRING_HASESCAPED => f.write_string_fast(w, self.str(), true)?,
            ARRAY => {
                f.begin_array(w)?;
                let len = self.len();
                let mut i = 0;
                let mut next = self.children();
                while i < len {
                    f.begin_array_value(w, i == 0)?;
                    next.serialize(w, f)?;
                    f.end_array_value(w)?;
                    i += 1;
                    next = next.next();
                }
                f.end_array(w)?;
            }
            OBJECT => {
                f.begin_object(w)?;
                let len = self.len();
                let mut i = 0;
                let mut next = self.children();
                while i < len {
                    let key = next;
                    f.begin_object_key(w, i == 0)?;
                    f.write_string_fast(w, key.str(), true)?;
                    f.end_object_key(w)?;

                    f.begin_object_value(w)?;
                    let val = key.next();
                    val.serialize(w, f)?;
                    f.end_object_value(w)?;
                    next = val.next();
                    i += 1;
                }
                f.end_object(w)?;
            }
            _ => unreachable!("unknow types in serialize {}", typ),
        }

        Ok(())
    }
}

pub fn dom_to_string(value: &Document) -> Result<String> {
    let mut buf = Vec::new();
    let mut format = crate::format::CompactFormatter;
    value
        .root()
        .serialize(&mut buf, &mut format)
        .map_err(crate::Error::io)?;

    Ok(unsafe { String::from_utf8_unchecked(buf) })
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct JsonStat {
    pub object: u32,
    pub array: u32,
    pub string: u32,
    pub number: u32,
    pub array_elems: u32,
    pub object_keys: u32,
    pub max_depth: u32,
}

#[derive(Debug, Default)]
pub struct Document {
    /// A continous flatten buffer for all JSON values
    pub node_buffer: Vec<Value>,
    /// clone from input json
    pub json_buffer: Vec<u8>,
    pub error_msg: String,
    pub error_pos: usize,
    /// if the input json has invalid utf8, we will use utf8_lossy and replace old string with new
    /// buffer
    pub has_utf8_lossy: bool,
    pub stats: JsonStat,
}

impl Document {
    const PADDING_SIZE: usize = 64;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn root(&self) -> &Value {
        &self.node_buffer[0]
    }

    fn allocate_json_buf(json: &[u8]) -> Vec<u8> {
        let len = json.len();
        // allocate the padding buffer for the input json
        let real_size = len + Self::PADDING_SIZE;
        let mut json_buf = Vec::with_capacity(real_size);
        json_buf.extend_from_slice(json);
        json_buf.extend_from_slice(&[b'x', b'"', b'x']);
        json_buf.extend_from_slice(&[61; 61]);
        json_buf
    }

    fn parse_bytes_impl(&mut self, json: &[u8], config: Config) -> Result<()> {
        let mut json_buf = Self::allocate_json_buf(json);
        let slice = PaddedSliceRead::new(json_buf.as_mut_slice());
        let mut parser = Parser::new_with(slice, config);

        // a simple wrapper for visitor
        #[derive(Debug)]
        struct DocumentVisitor<'a> {
            nodes: &'a mut Vec<Value>,
            parent: usize,
            depth: usize,
            stats: JsonStat,
        }

        impl<'a> DocumentVisitor<'a> {
            // the array and object's logic is same.
            fn visit_con_end(&mut self, len: usize) -> bool {
                let visitor = self;
                let parent = visitor.parent;
                let old = visitor.nodes[parent].val as usize;
                visitor.parent = old;
                let next = visitor.nodes.len() - parent;
                visitor.nodes[parent].val = (len as u64) | (next as u64) << CON_LEN_BITS;
                if visitor.depth as u32 > visitor.stats.max_depth {
                    visitor.stats.max_depth = visitor.depth as u32;
                }
                visitor.depth -= 1;
                true
            }

            fn push_node(&mut self, node: Value) -> bool {
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
            fn visit_bool_pos(&mut self, val: bool, pos: usize) -> bool {
                self.push_node(Value::new_bool(val, pos))
            }

            #[inline(always)]
            fn visit_f64_pos(&mut self, val: f64, pos: usize) -> bool {
                self.stats.number += 1;
                // # Safety
                // we have checked the f64 in parsing number.
                let node = unsafe { Value::new_f64_unchecked(val, pos) };
                self.push_node(node)
            }

            #[inline(always)]
            fn visit_i64_pos(&mut self, val: i64, pos: usize) -> bool {
                self.stats.number += 1;
                self.push_node(Value::new_i64(val, pos))
            }

            #[inline(always)]
            fn visit_u64_pos(&mut self, val: u64, pos: usize) -> bool {
                self.stats.number += 1;
                self.push_node(Value::new_u64(val, pos))
            }

            #[inline(always)]
            fn visit_number(&mut self, val: &str) -> bool {
                self.stats.number += 1;
                self.push_node(Value::new_number(val))
            }

            #[inline(always)]
            fn visit_array_start_pos(&mut self, _hint: usize, pos: usize) -> bool {
                let ret = self.push_node(Value::new_array(pos));
                // record the parent container position
                let len = self.nodes.len();
                self.nodes[len - 1].val = self.parent as u64;
                self.parent = len - 1;
                self.depth += 1;
                ret
            }

            #[inline(always)]
            fn visit_array_end(&mut self, len: usize) -> bool {
                self.stats.array += 1;
                self.stats.array_elems += len as u32;
                self.visit_con_end(len)
            }

            #[inline(always)]
            fn visit_object_start_pos(&mut self, _hint: usize, pos: usize) -> bool {
                let ret = self.push_node(Value::new_object(pos));
                let len = self.nodes.len();
                self.nodes[len - 1].val = self.parent as u64;
                self.parent = len - 1;
                self.depth += 1;
                ret
            }

            #[inline(always)]
            fn visit_object_end(&mut self, len: usize) -> bool {
                self.stats.object += 1;
                self.stats.object_keys += len as u32;
                self.visit_con_end(len)
            }

            #[inline(always)]
            fn visit_null_pos(&mut self, pos: usize) -> bool {
                self.push_node(Value::new_null(pos))
            }

            #[inline(always)]
            fn visit_str_status(&mut self, value: &str, has_escaped: bool) -> bool {
                self.stats.string += 1;
                self.push_node(Value::new_str(value, has_escaped))
            }

            #[inline(always)]
            fn visit_key_status(&mut self, value: &str, has_escaped: bool) -> bool {
                self.push_node(Value::new_str(value, has_escaped))
            }
        }

        // optimize: use a pre-allocated vec.
        // If json is valid, the max number of value nodes should be
        // half of the valid json length + 2. like as [1,2,3,1,2,3...]
        // if the capacity is not enough, we will return a error.
        let mut buf = Vec::with_capacity((json.len() / 2) + 2);
        let parent = 0;
        let mut visitor = DocumentVisitor {
            nodes: &mut buf,
            parent,
            stats: JsonStat::default(),
            depth: 0,
        };

        parser.parse_value(&mut visitor)?;
        // check trailing spaces
        parser.parse_trailing()?;

        self.json_buffer = json_buf;
        self.stats = visitor.stats;
        self.node_buffer = buf;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic() {
        // test parse and serialize
        let json = r#"
        {
            "name": "John Doe",
            "age": 43,
            "int": -1,
            "float": 1.2345,
            "phones": [
                "+44 1234567",
                "+44 2345678"
            ],
            "nested": {
                "key": [],
                "value": {},
                "array": [1,2,3,4,5]
            }
        }"#;
        let doc = unsafe { dom_from_slice_unchecked(json.as_bytes()).unwrap() };
        println!("stats:\n{:#?}", doc.stats);
        let out = dom_to_string(&doc).unwrap();
        println!("{out}");
    }

    #[test]
    fn test_parse_error() {
        let json = r#"{123}"#;
        let error = unsafe { dom_from_slice_unchecked(json.as_bytes()).unwrap_err() };
        println!("{error}");
    }

    #[test]
    fn test_parse_usenumber() {
        let json = r#"{"num1":123,"num2":[1,-3.14,0.0,-0]}"#;
        let config = Config::new().use_number(true);
        let doc = dom_from_slice_config(json.as_bytes(), config).unwrap();
        let out = dom_to_string(&doc).unwrap();
        assert_eq!(json, out);
        println!("{out}");
    }

    #[test]
    fn test_parse_utf8_lossy() {
        let json = b"\"Hello \xF0\x90\x80World\"";
        let err = dom_from_slice(json).unwrap_err();
        println!("{err}");

        let config = Config::new().validate_string(true);
        let doc = dom_from_slice_config(json, config).unwrap();
        let out = dom_to_string(&doc).unwrap();
        println!("{out}");
    }

    #[test]
    fn test_parse_utf16_lossy() {
        fn test_json(json: &str) {
            let _ = dom_from_slice(json.as_bytes()).map_err(|err| println!("{err}"));
            let config = Config::new().disable_surrogates_error(true);
            let doc = dom_from_slice_config(json.as_bytes(), config).unwrap();
            let out = dom_to_string(&doc).unwrap();
            println!("{out}");
        }

        let cases = [
            r#""hello \ud800 world""#,
            r#""hello \ud800\udc00 world""#,
            r#""hello \ud800\ud800 world""#,
            r#""hello \udc00 world""#,
        ];
        for case in cases {
            test_json(case);
        }
    }

    #[test]
    fn test_json_stat() {}
}
