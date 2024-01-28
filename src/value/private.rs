//! Not Public API. A simple flatten DOM from JSON.

use crate::{parser::Parser, reader::PaddedSliceRead, value::visitor::JsonVisitor, Result};

// JSON Value Type
const NULL: u64 = 0;
const BOOL: u64 = 2;
const FALSE: u64 = BOOL;
const TRUE: u64 = (1 << 3) | BOOL;
const NUMBER: u64 = 3;
const UINT: u64 = NUMBER;
const SINT: u64 = (1 << 3) | NUMBER;
const REAL: u64 = (2 << 3) | NUMBER;
const STRING: u64 = 4;
const STRING_COMMON: u64 = STRING;
const STRING_HASESCAPED: u64 = (1 << 3) | STRING;
const OBJECT: u64 = 6;
const ARRAY: u64 = 7;

/// JSON Type Mask
const POS_MASK: u64 = (!0) << 32;
const POS_BITS: u64 = 32;
const TYPE_MASK: u64 = 0xFF;
const TYPE_BITS: u64 = 8;
const LEN_MASK: u64 = (u32::MAX as u64) & (!TYPE_MASK);
const LEN_BITS: u64 = 24;

/// Value
/// Layout:
/// (low)
/// | type (8 bits) | reserved (24 bits) | pos (32 bits) |
/// |   number, string ptr, object/array ptr (64 bits)   |
#[repr(C)]
#[derive(Debug, Default)]
pub struct Value {
    typ: u64,
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
            typ: t | ((val.len() as u64) << TYPE_BITS),
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
            let len = (self.typ >> TYPE_BITS) as usize;
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len))
        }
    }

    fn serialize<W: crate::writer::WriteExt, F: crate::format::Formatter>(
        &self,
        w: &mut W,
        f: &mut F,
        next: &mut *const Self,
    ) -> std::io::Result<()> {
        *next = unsafe { &*(*next).offset(1) };
        let typ = self.typ & TYPE_MASK;
        match typ {
            NULL => f.write_null(w)?,
            FALSE => f.write_bool(w, false)?,
            TRUE => f.write_bool(w, true)?,
            UINT => f.write_u64(w, self.val)?,
            SINT => f.write_i64(w, self.val as i64)?,
            REAL => f.write_f64(w, f64::from_bits(self.val))?,
            STRING_COMMON | STRING_HASESCAPED => f.write_string_fast(w, self.str(), true)?,
            ARRAY => {
                f.begin_array(w)?;
                let len = self.val as usize;
                let mut i = 0;
                while i < len {
                    f.begin_array_value(w, i == 0)?;
                    let node = unsafe { &**next };
                    node.serialize(w, f, next)?;
                    f.end_array_value(w)?;
                    i += 1;
                }
                f.end_array(w)?;
            }
            OBJECT => {
                f.begin_object(w)?;
                let len = self.val as usize;
                let mut i = 0;
                while i < len {
                    let key = unsafe { &**next }.str();
                    f.begin_object_key(w, i == 0)?;
                    f.write_string_fast(w, key, true)?;
                    f.end_object_key(w)?;
                    *next = unsafe { &*(*next).offset(1) };
                    let val = unsafe { &**next };
                    f.begin_object_value(w)?;
                    val.serialize(w, f, next)?;
                    f.end_object_value(w)?;
                    i += 1;
                }
                f.end_object(w)?;
            }
            _ => unreachable!("unknow types in serialize {}", typ),
        }

        Ok(())
    }
}

pub fn dom_from_slice(json: &[u8]) -> Result<Document> {
    use crate::util::utf8::from_utf8;
    let json = {
        let json = from_utf8(json)?;
        json.as_bytes()
    };

    let mut doc = Document::default();
    doc.parse_bytes_impl(json)?;
    Ok(doc)
}

pub unsafe fn dom_from_slice_unchecked(json: &[u8]) -> Result<Document> {
    let mut doc = Document::default();
    doc.parse_bytes_impl(json)?;
    Ok(doc)
}

pub fn dom_to_string(value: &Document) -> Result<String> {
    let mut next = value.root() as *const Value;
    let mut buf = Vec::new();
    let mut format = crate::format::CompactFormatter;
    unsafe {
        (*next)
            .serialize(&mut buf, &mut format, &mut next)
            .map_err(crate::Error::io)?;
    }

    Ok(unsafe { String::from_utf8_unchecked(buf) })
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Document {
    /// A continous flatten buffer for all JSON values
    pub node_buffer: Vec<Value>,
    /// clone from input json
    pub json_buffer: Vec<u8>,
    pub error_msg: String,
    pub error_pos: usize,
}

impl Document {
    const PADDING_SIZE: usize = 64;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn root(&self) -> &Value {
        &self.node_buffer[0]
    }

    fn parse_bytes_impl(&mut self, json: &[u8]) -> Result<()> {
        let len = json.len();
        // allocate the padding buffer for the input json
        let real_size = len + Self::PADDING_SIZE;
        let mut json_buf = Vec::with_capacity(real_size);
        json_buf.extend_from_slice(json);
        json_buf.extend_from_slice(&[b'x', b'"', b'x']);
        json_buf.extend_from_slice(&[61; 61]);

        let slice = PaddedSliceRead::new(json_buf.as_mut_slice());
        let mut parser = Parser::new(slice);

        // a simple wrapper for visitor
        #[derive(Debug)]
        struct DocumentVisitor<'a> {
            nodes: &'a mut Vec<Value>,
            parent: usize,
        }

        impl<'a> DocumentVisitor<'a> {
            // the array and object's logic is same.
            fn visit_con_end(&mut self, len: usize) -> bool {
                let visitor = self;
                let parent = visitor.parent;
                let old = visitor.nodes[parent].val as usize;
                visitor.parent = old;
                visitor.nodes[parent].val = len as u64;
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
            fn visit_bool(&mut self, val: bool) -> bool {
                self.push_node(Value::from(val))
            }

            #[inline(always)]
            fn visit_f64_pos(&mut self, val: f64, pos: usize) -> bool {
                // # Safety
                // we have checked the f64 in parsing number.
                let node = unsafe { Value::new_f64_unchecked(val, pos) };
                self.push_node(node)
            }

            #[inline(always)]
            fn visit_i64_pos(&mut self, val: i64, pos: usize) -> bool {
                self.push_node(Value::new_i64(val, pos))
            }

            #[inline(always)]
            fn visit_u64_pos(&mut self, val: u64, pos: usize) -> bool {
                self.push_node(Value::new_u64(val, pos))
            }

            #[inline(always)]
            fn visit_array_start_pos(&mut self, _hint: usize, pos: usize) -> bool {
                let ret = self.push_node(Value::new_array(pos));
                // record the parent container position
                let len = self.nodes.len();
                self.nodes[len - 1].val = self.parent as u64;
                self.parent = len - 1;
                ret
            }

            #[inline(always)]
            fn visit_array_end(&mut self, len: usize) -> bool {
                self.visit_con_end(len)
            }

            #[inline(always)]
            fn visit_object_start_pos(&mut self, _hint: usize, pos: usize) -> bool {
                let ret = self.push_node(Value::new_object(pos));
                let len = self.nodes.len();
                self.nodes[len - 1].val = self.parent as u64;
                self.parent = len - 1;
                ret
            }

            #[inline(always)]
            fn visit_object_end(&mut self, len: usize) -> bool {
                self.visit_con_end(len)
            }

            #[inline(always)]
            fn visit_null(&mut self) -> bool {
                self.push_node(Value::from(()))
            }

            #[inline(always)]
            fn visit_str_status(&mut self, value: &str, has_escaped: bool) -> bool {
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
        };

        parser.parse_value(&mut visitor)?;
        // check trailing spaces
        parser.parse_trailing()?;

        self.json_buffer = json_buf;
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
        let out = dom_to_string(&doc).unwrap();
        println!("{out}");
    }

    #[test]
    fn test_parse_error() {
        let json = r#"{123}"#;
        let error = unsafe { dom_from_slice_unchecked(json.as_bytes()).unwrap_err() };
        println!("{error}");
    }
}
