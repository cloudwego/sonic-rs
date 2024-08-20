//! Deserialize JSON data to a Rust data structure.

// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.
use std::{marker::PhantomData, mem::ManuallyDrop, ptr::slice_from_raw_parts};

use serde::{
    de::{self, Expected, Unexpected},
    forward_to_deserialize_any,
};

use crate::{
    error::{
        Error,
        ErrorCode::{self, EofWhileParsing, RecursionLimitExceeded},
        Result,
    },
    parser::{as_str, ParseStatus, Parser},
    reader::{Read, Reader, Reference},
    util::num::ParserNumber,
    value::node::Value,
    JsonInput,
};

const MAX_ALLOWED_DEPTH: u8 = u8::MAX;

//////////////////////////////////////////////////////////////////////////////

/// A structure that deserializes JSON into Rust values.
pub struct Deserializer<R> {
    pub(crate) parser: Parser<R>,
    scratch: Vec<u8>,
    remaining_depth: u8,
}

// some functions only used for struct visitors.
impl<'de, R: Reader<'de>> Deserializer<R> {
    /// Create a new deserializer.
    pub fn new(read: R) -> Self {
        Self {
            parser: Parser::new(read),
            scratch: Vec::new(),
            remaining_depth: MAX_ALLOWED_DEPTH,
        }
    }

    /// Deserialize a JSON stream to a Rust data structure.
    ///
    /// It can be used repeatedly and we do not check trailing chars after deserilalized.
    ///
    /// # Example
    ///
    /// ```
    /// # use sonic_rs::{prelude::*, Value};
    ///
    /// use sonic_rs::Deserializer;
    ///
    /// let multiple_json = r#"{"a": 123, "b": "foo"} true [1, 2, 3] wrong chars"#;
    ///
    /// let mut deserializer = Deserializer::from_json(multiple_json);
    ///
    /// let val: Value = deserializer.deserialize().unwrap();
    /// assert_eq!(val["a"].as_i64().unwrap(), 123);
    /// assert_eq!(val["b"].as_str().unwrap(), "foo");
    ///
    /// let val: bool = deserializer.deserialize().unwrap();
    /// assert_eq!(val, true);
    ///
    /// let val: Vec<u8> = deserializer.deserialize().unwrap();
    /// assert_eq!(val, &[1, 2, 3]);
    ///
    /// // encounter the wrong chars in json
    /// assert!(deserializer.deserialize::<Value>().is_err());
    /// ```
    pub fn deserialize<T>(&mut self) -> Result<T>
    where
        T: de::Deserialize<'de>,
    {
        de::Deserialize::deserialize(self)
    }

    /// Convert Deserializer to a [`StreamDeserializer`].
    pub fn into_stream<T>(self) -> StreamDeserializer<'de, T, R> {
        StreamDeserializer {
            de: self,
            data: PhantomData,
            lifetime: PhantomData,
            is_ending: false,
        }
    }
}

impl<'de> Deserializer<Read<'de>> {
    /// Create a new deserializer from a json input [`JsonInput`].
    pub fn from_json<I: JsonInput<'de>>(input: I) -> Self {
        Self::new(Read::from(input))
    }

    /// Create a new deserializer from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &'de str) -> Self {
        Self::new(Read::from(s))
    }

    /// Create a new deserializer from a string slice.
    pub fn from_slice(s: &'de [u8]) -> Self {
        Self::new(Read::from(s))
    }
}

/// An iterator that deserializes a json stream into multiple `T` values.
///
/// # Example
///
/// ```
/// use sonic_rs::{prelude::*, Deserializer, Value};
///
/// let multiple_json = r#"{"a": 123, "b": "foo"} true [1, 2, 3] wrong chars"#;
///
/// let mut stream = Deserializer::from_json(multiple_json).into_stream::<Value>();
///
/// let val = stream.next().unwrap().unwrap();
/// assert_eq!(val["a"].as_i64().unwrap(), 123);
/// assert_eq!(val["b"].as_str().unwrap(), "foo");
///
/// let val = stream.next().unwrap().unwrap();
/// assert_eq!(val, true);
///
/// let val = stream.next().unwrap().unwrap();
/// assert_eq!(val, &[1, 2, 3]);
///
/// // encounter the wrong chars in json
/// assert!(stream.next().unwrap().is_err());
/// ```
pub struct StreamDeserializer<'de, T, R> {
    de: Deserializer<R>,
    data: PhantomData<T>,
    lifetime: PhantomData<&'de R>,
    is_ending: bool,
}

impl<'de, T, R> Iterator for StreamDeserializer<'de, T, R>
where
    T: de::Deserialize<'de>,
    R: Reader<'de>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_ending {
            return None;
        }
        let val: Result<T> = self.de.deserialize();
        if val.is_err() {
            self.is_ending = true;
        }
        Some(val)
    }
}

// We only use our own error type; no need for From conversions provided by the
// standard library's try! macro. This reduces lines of LLVM IR by 4%.
macro_rules! tri {
    ($e:expr $(,)?) => {
        match $e {
            Ok(val) => val,
            Err(err) => {
                return Err(err);
            }
        }
    };
}

pub(crate) use tri;

struct DepthGuard<'a, R> {
    de: &'a mut Deserializer<R>,
}

impl<'a, 'de, R: Reader<'de>> DepthGuard<'a, R> {
    fn guard(de: &'a mut Deserializer<R>) -> Result<Self> {
        de.remaining_depth -= 1;
        if de.remaining_depth == 0 {
            return Err(de.parser.error(RecursionLimitExceeded));
        }
        Ok(Self { de })
    }
}

impl<'a, R> Drop for DepthGuard<'a, R> {
    fn drop(&mut self) {
        self.de.remaining_depth += 1;
    }
}

impl ParserNumber {
    fn visit<'de, V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            ParserNumber::Float(x) => visitor.visit_f64(x),
            ParserNumber::Unsigned(x) => visitor.visit_u64(x),
            ParserNumber::Signed(x) => visitor.visit_i64(x),
        }
    }

    pub(crate) fn invalid_type(self, exp: &dyn Expected) -> Error {
        match self {
            ParserNumber::Float(x) => de::Error::invalid_type(Unexpected::Float(x), exp),
            ParserNumber::Unsigned(x) => de::Error::invalid_type(Unexpected::Unsigned(x), exp),
            ParserNumber::Signed(x) => de::Error::invalid_type(Unexpected::Signed(x), exp),
        }
    }
}

macro_rules! impl_deserialize_number {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: de::Visitor<'de>,
        {
            self.deserialize_number(visitor)
        }
    };
}

// some functions only used for struct visitors.
impl<'de, R: Reader<'de>> Deserializer<R> {
    pub(crate) fn deserialize_number<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(EofWhileParsing));
        };

        let value = match peek {
            c @ b'-' | c @ b'0'..=b'9' => tri!(self.parser.parse_number(c)).visit(visitor),
            _ => Err(self.peek_invalid_type(peek, &visitor)),
        };

        // fixed error position if not matched type
        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    #[cold]
    fn peek_invalid_type(&mut self, peek: u8, exp: &dyn Expected) -> Error {
        self.parser.peek_invalid_type(peek, exp)
    }

    fn end_seq(&mut self) -> Result<()> {
        self.parser.parse_array_end()
    }

    fn end_map(&mut self) -> Result<()> {
        match self.parser.skip_space() {
            Some(b'}') => Ok(()),
            Some(b',') => Err(self.parser.error(ErrorCode::TrailingComma)),
            Some(_) => Err(self.parser.error(ErrorCode::ExpectedObjectCommaOrEnd)),
            None => Err(self.parser.error(ErrorCode::EofWhileParsing)),
        }
    }

    fn scan_integer128(&mut self, buf: &mut String) -> Result<()> {
        match self.parser.read.peek() {
            Some(b'0') => {
                buf.push('0');
                // There can be only one leading '0'.
                if let Some(ch) = self.parser.read.peek() {
                    if ch.is_ascii_digit() {
                        return Err(self.parser.error(ErrorCode::InvalidNumber));
                    }
                }
                self.parser.read.eat(1);
                Ok(())
            }
            Some(c) if c.is_ascii_digit() => {
                buf.push(c as char);
                self.parser.read.eat(1);
                while let c @ b'0'..=b'9' = self.parser.read.peek().unwrap_or_default() {
                    self.parser.read.eat(1);
                    buf.push(c as char);
                }
                Ok(())
            }
            _ => Err(self.parser.error(ErrorCode::InvalidNumber)),
        }
    }

    fn deserialize_lazyvalue<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let (raw, status) = self.parser.skip_one()?;
        if status == ParseStatus::HasEscaped {
            visitor.visit_str(as_str(raw))
        } else {
            visitor.visit_borrowed_str(as_str(raw))
        }
    }

    fn deserialize_value<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let shared = self.parser.get_shared_inc_count();
        let mut val = Value::new_null(shared.data_ptr());
        let val = if self.parser.read.index() == 0 {
            // get n to check trailing characters in later
            let n = val.parse_with_padding(self.parser.read.as_u8_slice())?;
            self.parser.read.eat(n);
            val
        } else {
            // deserialize some json parts into `Value`, not use padding buffer, avoid the memory
            // copy
            val.parse_without_padding(&mut self.parser)?;
            val
        };

        // deserialize `Value` must be root node
        debug_assert!(val.is_static() || val.is_root());
        if !val.shared_parts().is_null() {
            std::mem::forget(shared);
        }

        let val = ManuallyDrop::new(val);
        // #Safety
        // the json is validate before parsing json, and we pass the document using visit_bytes
        // here.
        unsafe {
            let binary =
                &*slice_from_raw_parts(&val as *const _ as *const u8, std::mem::size_of::<Value>());
            visitor.visit_bytes(binary)
        }
    }

    // we deserialize json number from string or number types
    fn deserialize_rawnumber<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let raw = match self.parser.skip_space_peek() {
            Some(c @ b'-' | c @ b'0'..=b'9') => {
                let start = self.parser.read.index();
                self.parser.read.eat(1);
                self.parser.skip_number(c)?;
                let end = self.parser.read.index();
                as_str(self.parser.read.slice_unchecked(start, end))
            }
            Some(b'"') => {
                self.parser.read.eat(1);
                let start = self.parser.read.index();
                match self.parser.read.next() {
                    Some(c @ b'-' | c @ b'0'..=b'9') => {
                        self.parser.skip_number(c)?;
                    }
                    _ => return Err(self.parser.error(ErrorCode::InvalidNumber)),
                }
                let end = self.parser.read.index();
                let raw = as_str(self.parser.read.slice_unchecked(start, end));
                // match the right quote
                if self.parser.read.next() != Some(b'"') {
                    return Err(self.parser.error(ErrorCode::InvalidNumber));
                }
                raw
            }
            _ => return Err(self.parser.error(ErrorCode::InvalidNumber)),
        };

        visitor.visit_borrowed_str(raw)
    }
}

impl<'de, 'a, R: Reader<'de>> de::Deserializer<'de> for &'a mut Deserializer<R> {
    type Error = Error;
    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(EofWhileParsing));
        };

        let value = match peek {
            b'n' => {
                tri!(self.parser.parse_literal("ull"));
                visitor.visit_unit()
            }
            b't' => {
                tri!(self.parser.parse_literal("rue"));
                visitor.visit_bool(true)
            }
            b'f' => {
                tri!(self.parser.parse_literal("alse"));
                visitor.visit_bool(false)
            }
            c @ b'-' | c @ b'0'..=b'9' => tri!(self.parser.parse_number(c)).visit(visitor),
            b'"' => {
                self.scratch.clear();
                match tri!(self.parser.parse_str_impl(&mut self.scratch)) {
                    Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
                    Reference::Copied(s) => visitor.visit_str(s),
                }
            }
            b'[' => {
                let ret = {
                    let _ = DepthGuard::guard(self);
                    visitor.visit_seq(SeqAccess::new(self))
                };
                match (ret, self.end_seq()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            b'{' => {
                let ret = {
                    let _ = DepthGuard::guard(self);
                    visitor.visit_map(MapAccess::new(self))
                };
                match (ret, self.end_map()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            _ => Err(self.parser.error(ErrorCode::InvalidJsonValue)),
        };

        match value {
            Ok(value) => Ok(value),
            // The de::Error impl creates errors with unknown line and column.
            // Fill in the position here by looking at the current index in the
            // input. There is no way to tell whether this should call `error`
            // or `error` so pick the one that seems correct more often.
            // Worst case, the position is off by one character.
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let value = match peek {
            b't' => {
                tri!(self.parser.parse_literal("rue"));
                visitor.visit_bool(true)
            }
            b'f' => {
                tri!(self.parser.parse_literal("alse"));
                visitor.visit_bool(false)
            }
            _ => Err(self.peek_invalid_type(peek, &visitor)),
        };

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    impl_deserialize_number!(deserialize_i8);
    impl_deserialize_number!(deserialize_i16);
    impl_deserialize_number!(deserialize_i32);
    impl_deserialize_number!(deserialize_i64);
    impl_deserialize_number!(deserialize_u8);
    impl_deserialize_number!(deserialize_u16);
    impl_deserialize_number!(deserialize_u32);
    impl_deserialize_number!(deserialize_u64);
    impl_deserialize_number!(deserialize_f32);
    impl_deserialize_number!(deserialize_f64);

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let mut buf = String::new();
        match self.parser.skip_space_peek() {
            Some(b'-') => {
                buf.push('-');
                self.parser.read.eat(1);
            }
            Some(_) => {}
            None => {
                return Err(self.parser.error(ErrorCode::EofWhileParsing));
            }
        };

        tri!(self.scan_integer128(&mut buf));

        let value = match buf.parse() {
            Ok(int) => visitor.visit_i128(int),
            Err(_) => {
                return Err(self.parser.error(ErrorCode::NumberOutOfRange));
            }
        };

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parser.skip_space_peek() {
            Some(b'-') => {
                return Err(self.parser.error(ErrorCode::NumberOutOfRange));
            }
            Some(_) => {}
            None => {
                return Err(self.parser.error(ErrorCode::EofWhileParsing));
            }
        }

        let mut buf = String::new();
        tri!(self.scan_integer128(&mut buf));

        let value = match buf.parse() {
            Ok(int) => visitor.visit_u128(int),
            Err(_) => {
                return Err(self.parser.error(ErrorCode::NumberOutOfRange));
            }
        };

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let value = match peek {
            b'"' => {
                self.scratch.clear();
                match tri!(self.parser.parse_str_impl(&mut self.scratch)) {
                    Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
                    Reference::Copied(s) => visitor.visit_str(s),
                }
            }
            _ => Err(self.peek_invalid_type(peek, &visitor)),
        };

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Parses a JSON string as bytes. Note that this function does not check
    /// whether the bytes represent a valid UTF-8 string.
    ///
    /// Followed as `serde_json`.
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let start = self.parser.read.index();
        let value = match peek {
            b'"' => match tri!(self.parser.parse_string_raw(&mut self.scratch)) {
                Reference::Borrowed(b) => visitor.visit_borrowed_bytes(b),
                Reference::Copied(b) => visitor.visit_bytes(b),
            },
            b'[' => {
                self.parser.read.backward(1);
                self.deserialize_seq(visitor)
            }
            _ => Err(self.peek_invalid_type(peek, &visitor)),
        };

        // check invalid utf8 with allow space here
        self.parser
            .read
            .validate_utf8((start, self.parser.read.index()))?;
        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    /// Parses a `null` as a None, and any other values as a `Some(...)`.
    #[inline]
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parser.skip_space_peek() {
            Some(b'n') => {
                self.parser.read.eat(1);
                tri!(self.parser.parse_literal("ull"));
                visitor.visit_none()
            }
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let value = match peek {
            b'n' => {
                tri!(self.parser.parse_literal("ull"));
                visitor.visit_unit()
            }
            _ => Err(self.peek_invalid_type(peek, &visitor)),
        };

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    /// Parses a newtype struct as the underlying value.
    #[inline]
    fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        {
            if name == crate::serde::rawnumber::TOKEN {
                return self.deserialize_rawnumber(visitor);
            } else if name == crate::lazyvalue::TOKEN {
                return self.deserialize_lazyvalue(visitor);
            } else if name == crate::value::de::TOKEN {
                return self.deserialize_value(visitor);
            }
        }

        let _ = name;
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let value = match peek {
            b'[' => {
                let ret = {
                    let _ = DepthGuard::guard(self);
                    visitor.visit_seq(SeqAccess::new(self))
                };
                match (ret, self.end_seq()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            _ => return Err(self.peek_invalid_type(peek, &visitor)),
        };
        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let value = match peek {
            b'{' => {
                let ret = {
                    let _ = DepthGuard::guard(self);
                    visitor.visit_map(MapAccess::new(self))
                };
                match (ret, self.end_map()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            _ => return Err(self.peek_invalid_type(peek, &visitor)),
        };
        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let Some(peek) = self.parser.skip_space() else {
            return Err(self.parser.error(ErrorCode::EofWhileParsing));
        };

        let value = match peek {
            b'[' => {
                let ret = {
                    let _ = DepthGuard::guard(self);
                    visitor.visit_seq(SeqAccess::new(self))
                };
                match (ret, self.end_seq()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            b'{' => {
                let ret = {
                    let _ = DepthGuard::guard(self);
                    visitor.visit_map(MapAccess::new(self))
                };
                match (ret, self.end_map()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            _ => return Err(self.peek_invalid_type(peek, &visitor)),
        };

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.parser.fix_position(err)),
        }
    }

    /// Parses an enum as an object like `{"$KEY":$VALUE}`, where $VALUE is either a straight
    /// value, a `[..]`, or a `{..}`.
    #[inline]
    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parser.skip_space_peek() {
            Some(b'{') => {
                self.parser.read.eat(1);
                let value = {
                    let _ = DepthGuard::guard(self);
                    tri!(visitor.visit_enum(VariantAccess::new(self)))
                };

                match self.parser.skip_space() {
                    Some(b'}') => Ok(value),
                    Some(_) => Err(self.parser.error(ErrorCode::InvalidJsonValue)),
                    None => Err(self.parser.error(ErrorCode::EofWhileParsing)),
                }
            }
            Some(b'"') => visitor.visit_enum(UnitVariantAccess::new(self)),
            Some(_) => Err(self.parser.error(ErrorCode::InvalidJsonValue)),
            None => Err(self.parser.error(ErrorCode::EofWhileParsing)),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        // NOTE: we use faster skip, and will not validate the skipped parts.
        tri!(self.parser.skip_one());
        visitor.visit_unit()
    }
}

struct SeqAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
    first: bool, // first is marked as
}

impl<'a, R: 'a> SeqAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        SeqAccess { de, first: true }
    }
}

impl<'de, 'a, R: Reader<'de> + 'a> de::SeqAccess<'de> for SeqAccess<'a, R> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.de.parser.skip_space_peek() {
            Some(b']') => Ok(None), // we will check the ending brace after `visit_seq`
            Some(b',') if !self.first => {
                self.de.parser.read.eat(1);
                Ok(Some(tri!(seed.deserialize(&mut *self.de))))
            }
            Some(_) => {
                if self.first {
                    self.first = false;
                    Ok(Some(tri!(seed.deserialize(&mut *self.de))))
                } else {
                    self.de.parser.read.eat(1); // makes the error position is correct
                    Err(self.de.parser.error(ErrorCode::ExpectedArrayCommaOrEnd))
                }
            }
            None => Err(self.de.parser.error(ErrorCode::EofWhileParsing)),
        }
    }
}

struct MapAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
    first: bool,
}

impl<'a, R: 'a> MapAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        MapAccess { de, first: true }
    }
}

impl<'de, 'a, R: Reader<'de> + 'a> de::MapAccess<'de> for MapAccess<'a, R> {
    type Error = Error;

    #[inline(always)]
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        let peek = match self.de.parser.skip_space_peek() {
            Some(b'}') => {
                return Ok(None);
            }
            Some(b',') if !self.first => {
                self.de.parser.read.eat(1);
                self.de.parser.skip_space()
            }
            Some(b) => {
                self.de.parser.read.eat(1);
                if self.first {
                    self.first = false;
                    Some(b)
                } else {
                    return Err(self.de.parser.error(ErrorCode::ExpectedObjectCommaOrEnd));
                }
            }
            None => {
                return Err(self.de.parser.error(ErrorCode::EofWhileParsing));
            }
        };

        match peek {
            Some(b'"') => seed.deserialize(MapKey { de: &mut *self.de }).map(Some),
            Some(b'}') => Err(self.de.parser.error(ErrorCode::TrailingComma)),
            Some(_) => Err(self.de.parser.error(ErrorCode::ExpectObjectKeyOrEnd)),
            None => Err(self.de.parser.error(ErrorCode::EofWhileParsing)),
        }
    }

    #[inline(always)]
    fn next_value<V>(&mut self) -> Result<V>
    where
        V: de::Deserialize<'de>,
    {
        use std::marker::PhantomData;
        self.next_value_seed(PhantomData)
    }

    #[inline(always)]
    fn next_entry<K, V>(&mut self) -> Result<Option<(K, V)>>
    where
        K: de::Deserialize<'de>,
        V: de::Deserialize<'de>,
    {
        use std::marker::PhantomData;
        self.next_entry_seed(PhantomData, PhantomData)
    }

    #[inline(always)]
    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        tri!(self.de.parser.parse_object_clo());
        seed.deserialize(&mut *self.de)
    }
}

struct VariantAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R: 'a> VariantAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        VariantAccess { de }
    }
}

impl<'de, 'a, R: Reader<'de> + 'a> de::EnumAccess<'de> for VariantAccess<'a, R> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
    where
        V: de::DeserializeSeed<'de>,
    {
        let val = tri!(seed.deserialize(&mut *self.de));
        tri!(self.de.parser.parse_object_clo());
        Ok((val, self))
    }
}

impl<'de, 'a, R: Reader<'de> + 'a> de::VariantAccess<'de> for VariantAccess<'a, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        de::Deserialize::deserialize(self.de)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(self.de, "", fields, visitor)
    }
}

struct UnitVariantAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R: 'a> UnitVariantAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        UnitVariantAccess { de }
    }
}

impl<'de, 'a, R: Reader<'de> + 'a> de::EnumAccess<'de> for UnitVariantAccess<'a, R> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = tri!(seed.deserialize(&mut *self.de));
        Ok((variant, self))
    }
}

impl<'de, 'a, R: Reader<'de> + 'a> de::VariantAccess<'de> for UnitVariantAccess<'a, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"newtype variant",
        ))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"tuple variant",
        ))
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"struct variant",
        ))
    }
}

/// Only deserialize from this after peeking a '"' byte! Otherwise it may
/// deserialize invalid JSON successfully./// Only deserialize from this after peeking a '"' byte!
/// Otherwise it may deserialize invalid JSON successfully.
struct MapKey<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

macro_rules! deserialize_numeric_key {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: de::Visitor<'de>,
        {
            let value = tri!(self.de.deserialize_number(visitor));
            if self.de.parser.read.next() != Some(b'"') {
                return Err(self.de.parser.error(ErrorCode::ExpectedQuote));
            }

            Ok(value)
        }
    };

    ($method:ident, $delegate:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: de::Visitor<'de>,
        {
            match self.de.parser.read.peek() {
                Some(b'0'..=b'9' | b'-') => {}
                _ => return Err(self.de.parser.error(ErrorCode::ExpectedNumericKey)),
            }

            let value = tri!(self.de.$delegate(visitor));

            if self.de.parser.read.next() != Some(b'"') {
                return Err(self.de.parser.error(ErrorCode::ExpectedQuote));
            }

            Ok(value)
        }
    };
}

impl<'de, 'a, R> MapKey<'a, R>
where
    R: Reader<'de>,
{
    fn deserialize_number<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.de.parser.read.peek() {
            Some(b'0'..=b'9' | b'-') => {}
            _ => return Err(self.de.parser.error(ErrorCode::ExpectedNumericKey)),
        }

        let value = tri!(self.de.deserialize_number(visitor));

        if self.de.parser.read.next() != Some(b'"') {
            return Err(self.de.parser.error(ErrorCode::ExpectedQuote));
        }

        Ok(value)
    }
}

impl<'de, 'a, R> de::Deserializer<'de> for MapKey<'a, R>
where
    R: Reader<'de>,
{
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.de.scratch.clear();
        match tri!(self.de.parser.parse_str_impl(&mut self.de.scratch)) {
            Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
            Reference::Copied(s) => visitor.visit_str(s),
        }
    }

    deserialize_numeric_key!(deserialize_i8);
    deserialize_numeric_key!(deserialize_i16);
    deserialize_numeric_key!(deserialize_i32);
    deserialize_numeric_key!(deserialize_i64);
    deserialize_numeric_key!(deserialize_i128, deserialize_i128);
    deserialize_numeric_key!(deserialize_u8);
    deserialize_numeric_key!(deserialize_u16);
    deserialize_numeric_key!(deserialize_u32);
    deserialize_numeric_key!(deserialize_u64);
    deserialize_numeric_key!(deserialize_u128, deserialize_u128);
    deserialize_numeric_key!(deserialize_f32);
    deserialize_numeric_key!(deserialize_f64);

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let mut value = match self.de.parser.read.next() {
            Some(b't') => {
                tri!(self.de.parser.parse_literal("rue"));
                visitor.visit_bool(true)
            }
            Some(b'f') => {
                tri!(self.de.parser.parse_literal("alse"));
                visitor.visit_bool(false)
            }
            None => Err(self.de.parser.error(ErrorCode::EofWhileParsing)),
            Some(peek) => Err(self.de.peek_invalid_type(peek, &visitor)),
        };

        if self.de.parser.read.next() != Some(b'"') {
            value = Err(self.de.parser.error(ErrorCode::ExpectedQuote));
        }

        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(self.de.parser.fix_position(err)),
        }
    }

    #[inline]
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        // Map keys cannot be null.
        visitor.visit_some(self)
    }

    #[inline]
    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    #[inline]
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.de.parser.read.backward(1);
        self.de.deserialize_enum(name, variants, visitor)
    }

    #[inline]
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.de.parser.read.backward(1);
        self.de.deserialize_bytes(visitor)
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.de.parser.read.backward(1);
        self.de.deserialize_bytes(visitor)
    }

    forward_to_deserialize_any! {
        char str string unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}
//////////////////////////////////////////////////////////////////////////////

fn from_trait<'de, R, T>(read: R) -> Result<T>
where
    R: Reader<'de>,
    T: de::Deserialize<'de>,
{
    let mut de = Deserializer::new(read);
    let value = tri!(de::Deserialize::deserialize(&mut de));

    // Make sure the whole stream has been consumed.
    tri!(de.parser.parse_trailing());

    // check invalid utf8
    tri!(de.parser.read.check_utf8_final());
    Ok(value)
}

/// Deserialize an instance of type `T` from bytes of JSON text.
/// If user can guarantee the JSON is valid UTF-8, recommend to use `from_slice_unchecked` instead.
pub fn from_slice<'a, T>(json: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    from_trait(Read::new(json, true))
}

/// Deserialize an instance of type `T` from bytes of JSON text.
///
/// # Safety
/// The json passed in must be valid UTF-8.
pub unsafe fn from_slice_unchecked<'a, T>(json: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    from_trait(Read::new(json, false))
}

/// Deserialize an instance of type `T` from a string of JSON text.
pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    from_trait(Read::new(s.as_bytes(), false))
}
