//! Serialize a Rust data structure into JSON data.

// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.

use crate::error::{Error, ErrorCode, Result};
use crate::format::{CompactFormatter, Formatter, PrettyFormatter};
use crate::serde::tri;
use crate::writer::WriterExt;
use core::fmt::{self, Display};
use core::num::FpCategory;
use serde::ser::{self, Impossible, Serialize};
use std::io;
use std::string::{String, ToString};
use std::vec::Vec;

/// A structure for serializing Rust values into JSON.
pub struct Serializer<W, F = CompactFormatter> {
    writer: W,
    formatter: F,
}

impl<W> Serializer<W>
where
    W: WriterExt,
{
    /// Creates a new JSON serializer.
    #[inline]
    pub fn new(writer: W) -> Self {
        Serializer::with_formatter(writer, CompactFormatter)
    }
}

impl<'a, W> Serializer<W, PrettyFormatter<'a>>
where
    W: WriterExt,
{
    /// Creates a new JSON pretty print serializer.
    #[inline]
    pub fn pretty(writer: W) -> Self {
        Serializer::with_formatter(writer, PrettyFormatter::new())
    }
}

impl<W, F> Serializer<W, F>
where
    W: WriterExt,
    F: Formatter,
{
    /// Creates a new JSON visitor whose output will be written to the writer
    /// specified.
    #[inline]
    pub fn with_formatter(writer: W, formatter: F) -> Self {
        Serializer { writer, formatter }
    }

    /// Unwrap the `Writer` from the `Serializer`.
    #[inline]
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<'a, W, F> ser::Serializer for &'a mut Serializer<W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Compound<'a, W, F>;
    type SerializeTuple = Compound<'a, W, F>;
    type SerializeTupleStruct = Compound<'a, W, F>;
    type SerializeTupleVariant = Compound<'a, W, F>;
    type SerializeMap = Compound<'a, W, F>;
    type SerializeStruct = Compound<'a, W, F>;
    type SerializeStructVariant = Compound<'a, W, F>;

    #[inline]
    fn serialize_bool(self, value: bool) -> Result<()> {
        self.formatter
            .write_bool(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_i8(self, value: i8) -> Result<()> {
        self.formatter
            .write_i8(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_i16(self, value: i16) -> Result<()> {
        self.formatter
            .write_i16(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_i32(self, value: i32) -> Result<()> {
        self.formatter
            .write_i32(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_i64(self, value: i64) -> Result<()> {
        self.formatter
            .write_i64(&mut self.writer, value)
            .map_err(Error::io)
    }

    fn serialize_i128(self, value: i128) -> Result<()> {
        self.formatter
            .write_i128(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_u8(self, value: u8) -> Result<()> {
        self.formatter
            .write_u8(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_u16(self, value: u16) -> Result<()> {
        self.formatter
            .write_u16(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_u32(self, value: u32) -> Result<()> {
        self.formatter
            .write_u32(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_u64(self, value: u64) -> Result<()> {
        self.formatter
            .write_u64(&mut self.writer, value)
            .map_err(Error::io)
    }

    fn serialize_u128(self, value: u128) -> Result<()> {
        self.formatter
            .write_u128(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_f32(self, value: f32) -> Result<()> {
        match value.classify() {
            FpCategory::Nan | FpCategory::Infinite => self
                .formatter
                .write_null(&mut self.writer)
                .map_err(Error::io),
            _ => self
                .formatter
                .write_f32(&mut self.writer, value)
                .map_err(Error::io),
        }
    }

    #[inline]
    fn serialize_f64(self, value: f64) -> Result<()> {
        match value.classify() {
            FpCategory::Nan | FpCategory::Infinite => self
                .formatter
                .write_null(&mut self.writer)
                .map_err(Error::io),
            _ => self
                .formatter
                .write_f64(&mut self.writer, value)
                .map_err(Error::io),
        }
    }

    #[inline]
    fn serialize_char(self, value: char) -> Result<()> {
        // A char encoded as UTF-8 takes 4 bytes at most.
        let mut buf = [0; 4];
        self.serialize_str(value.encode_utf8(&mut buf))
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        self.formatter
            .write_string_fast(&mut self.writer, value)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_bytes(self, value: &[u8]) -> Result<()> {
        use serde::ser::SerializeSeq;
        let mut seq = tri!(self.serialize_seq(Some(value.len())));
        for byte in value {
            tri!(seq.serialize_element(byte));
        }
        seq.end()
    }

    #[inline]
    fn serialize_unit(self) -> Result<()> {
        self.formatter
            .write_null(&mut self.writer)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    /// Serialize newtypes without an object wrapper.
    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        tri!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        tri!(self
            .formatter
            .begin_object_key(&mut self.writer, true)
            .map_err(Error::io));
        tri!(self.serialize_str(variant));
        tri!(self
            .formatter
            .end_object_key(&mut self.writer)
            .map_err(Error::io));
        tri!(self
            .formatter
            .begin_object_value(&mut self.writer)
            .map_err(Error::io));
        tri!(value.serialize(&mut *self));
        tri!(self
            .formatter
            .end_object_value(&mut self.writer)
            .map_err(Error::io));
        self.formatter
            .end_object(&mut self.writer)
            .map_err(Error::io)
    }

    #[inline]
    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        tri!(self
            .formatter
            .begin_array(&mut self.writer)
            .map_err(Error::io));
        if len == Some(0) {
            tri!(self
                .formatter
                .end_array(&mut self.writer)
                .map_err(Error::io));
            Ok(Compound::Map {
                ser: self,
                state: State::Empty,
            })
        } else {
            Ok(Compound::Map {
                ser: self,
                state: State::First,
            })
        }
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        tri!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        tri!(self
            .formatter
            .begin_object_key(&mut self.writer, true)
            .map_err(Error::io));
        tri!(self.serialize_str(variant));
        tri!(self
            .formatter
            .end_object_key(&mut self.writer)
            .map_err(Error::io));
        tri!(self
            .formatter
            .begin_object_value(&mut self.writer)
            .map_err(Error::io));
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        tri!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        if len == Some(0) {
            tri!(self
                .formatter
                .end_object(&mut self.writer)
                .map_err(Error::io));
            Ok(Compound::Map {
                ser: self,
                state: State::Empty,
            })
        } else {
            Ok(Compound::Map {
                ser: self,
                state: State::First,
            })
        }
    }

    #[inline]
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        match name {
            crate::serde::raw::TOKEN => Ok(Compound::RawValue { ser: self }),
            crate::serde::number::TOKEN => Ok(Compound::RawValue { ser: self }),
            _ => self.serialize_map(Some(len)),
        }
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        tri!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        tri!(self
            .formatter
            .begin_object_key(&mut self.writer, true)
            .map_err(Error::io));
        tri!(self.serialize_str(variant));
        tri!(self
            .formatter
            .end_object_key(&mut self.writer)
            .map_err(Error::io));
        tri!(self
            .formatter
            .begin_object_value(&mut self.writer)
            .map_err(Error::io));
        self.serialize_map(Some(len))
    }

    // Serialize a string produced by an implementation of Display. such as DataTime
    fn collect_str<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Display,
    {
        use self::fmt::Write;

        struct Adapter<'ser, W: 'ser, F: 'ser> {
            writer: &'ser mut W,
            formatter: &'ser mut F,
            error: Option<io::Error>,
        }

        impl<'ser, W, F> Write for Adapter<'ser, W, F>
        where
            W: WriterExt,
            F: Formatter,
        {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                debug_assert!(self.error.is_none());
                match self.formatter.write_string_fast(self.writer, s) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        self.error = Some(err);
                        Err(fmt::Error)
                    }
                }
            }
        }

        let mut adapter = Adapter {
            writer: &mut self.writer,
            formatter: &mut self.formatter,
            error: None,
        };
        match write!(adapter, "{}", value) {
            Ok(()) => {
                debug_assert!(adapter.error.is_none());
                Ok(())
            }
            Err(fmt::Error) => Err(Error::io(adapter.error.expect("there should be an error"))),
        }
    }
}

// Not public API. Should be pub(crate).
#[doc(hidden)]
#[derive(Eq, PartialEq)]
pub enum State {
    Empty,
    First,
    Rest,
}

// Not public API. Should be pub(crate).
#[doc(hidden)]
pub enum Compound<'a, W: 'a, F: 'a> {
    Map {
        ser: &'a mut Serializer<W, F>,
        state: State,
    },

    RawValue {
        ser: &'a mut Serializer<W, F>,
    },
}

impl<'a, W, F> ser::SerializeSeq for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match self {
            Compound::Map { ser, state } => {
                tri!(ser
                    .formatter
                    .begin_array_value(&mut ser.writer, *state == State::First)
                    .map_err(Error::io));
                *state = State::Rest;
                tri!(value.serialize(&mut **ser));
                ser.formatter
                    .end_array_value(&mut ser.writer)
                    .map_err(Error::io)
            }

            Compound::RawValue { .. } => unreachable!(),
        }
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self {
            Compound::Map { ser, state } => match state {
                State::Empty => Ok(()),
                _ => ser.formatter.end_array(&mut ser.writer).map_err(Error::io),
            },

            Compound::RawValue { .. } => unreachable!(),
        }
    }
}

impl<'a, W, F> ser::SerializeTuple for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W, F> ser::SerializeTupleStruct for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W, F> ser::SerializeTupleVariant for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self {
            Compound::Map { ser, state } => {
                match state {
                    State::Empty => {}
                    _ => tri!(ser.formatter.end_array(&mut ser.writer).map_err(Error::io)),
                }
                tri!(ser
                    .formatter
                    .end_object_value(&mut ser.writer)
                    .map_err(Error::io));
                ser.formatter.end_object(&mut ser.writer).map_err(Error::io)
            }

            Compound::RawValue { .. } => unreachable!(),
        }
    }
}

impl<'a, W, F> ser::SerializeMap for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match self {
            Compound::Map { ser, state } => {
                tri!(ser
                    .formatter
                    .begin_object_key(&mut ser.writer, *state == State::First)
                    .map_err(Error::io));
                *state = State::Rest;

                tri!(key.serialize(MapKeySerializer { ser: *ser }));

                ser.formatter
                    .end_object_key(&mut ser.writer)
                    .map_err(Error::io)
            }

            Compound::RawValue { .. } => unreachable!(),
        }
    }

    #[inline]
    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match self {
            Compound::Map { ser, .. } => {
                tri!(ser
                    .formatter
                    .begin_object_value(&mut ser.writer)
                    .map_err(Error::io));
                tri!(value.serialize(&mut **ser));
                ser.formatter
                    .end_object_value(&mut ser.writer)
                    .map_err(Error::io)
            }

            Compound::RawValue { .. } => unreachable!(),
        }
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self {
            Compound::Map { ser, state } => match state {
                State::Empty => Ok(()),
                _ => ser.formatter.end_object(&mut ser.writer).map_err(Error::io),
            },

            Compound::RawValue { .. } => unreachable!(),
        }
    }
}

impl<'a, W, F> ser::SerializeStruct for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match self {
            Compound::Map { .. } => ser::SerializeMap::serialize_entry(self, key, value),

            Compound::RawValue { ser, .. } => {
                if key == crate::serde::raw::TOKEN {
                    value.serialize(RawValueStrEmitter(ser))
                } else {
                    Err(invalid_raw_value())
                }
            }
        }
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self {
            Compound::Map { .. } => ser::SerializeMap::end(self),

            Compound::RawValue { .. } => Ok(()),
        }
    }
}

impl<'a, W, F> ser::SerializeStructVariant for Compound<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match *self {
            Compound::Map { .. } => ser::SerializeStruct::serialize_field(self, key, value),

            Compound::RawValue { .. } => unreachable!(),
        }
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self {
            Compound::Map { ser, state } => {
                match state {
                    State::Empty => {}
                    _ => tri!(ser.formatter.end_object(&mut ser.writer).map_err(Error::io)),
                }
                tri!(ser
                    .formatter
                    .end_object_value(&mut ser.writer)
                    .map_err(Error::io));
                ser.formatter.end_object(&mut ser.writer).map_err(Error::io)
            }

            Compound::RawValue { .. } => unreachable!(),
        }
    }
}

struct MapKeySerializer<'a, W: 'a, F: 'a> {
    ser: &'a mut Serializer<W, F>,
}

fn invalid_raw_value() -> Error {
    Error::syntax(ErrorCode::InvalidJsonValue, 0, 0)
}

fn key_must_be_a_string() -> Error {
    Error::syntax(ErrorCode::ExpectObjectKeyOrEnd, 0, 0)
}

impl<'a, W, F> ser::Serializer for MapKeySerializer<'a, W, F>
where
    W: WriterExt,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        self.ser.serialize_str(value)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.ser.serialize_str(variant)
    }

    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_bool(self, _value: bool) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_i8(self, value: i8) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_i8(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_i16(self, value: i16) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_i16(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_i32(self, value: i32) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_i32(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_i64(self, value: i64) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_i64(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_i128(self, value: i128) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_i128(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_u8(self, value: u8) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_u8(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_u16(self, value: u16) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_u16(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_u32(self, value: u32) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_u32(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_u64(self, value: u64) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_u64(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_u128(self, value: u128) -> Result<()> {
        tri!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        tri!(self
            .ser
            .formatter
            .write_u128(&mut self.ser.writer, value)
            .map_err(Error::io));
        self.ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io)
    }

    fn serialize_f32(self, _value: f32) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_f64(self, _value: f64) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_char(self, value: char) -> Result<()> {
        self.ser.serialize_str(&value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_unit(self) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(key_must_be_a_string())
    }

    fn serialize_none(self) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(key_must_be_a_string())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(key_must_be_a_string())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(key_must_be_a_string())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(key_must_be_a_string())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(key_must_be_a_string())
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(key_must_be_a_string())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(key_must_be_a_string())
    }

    fn collect_str<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Display,
    {
        self.ser.collect_str(value)
    }
}

struct RawValueStrEmitter<'a, W: 'a + WriterExt, F: 'a + Formatter>(&'a mut Serializer<W, F>);

impl<'a, W: WriterExt, F: Formatter> ser::Serializer for RawValueStrEmitter<'a, W, F> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_bool(self, _v: bool) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_i8(self, _v: i8) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_i16(self, _v: i16) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_i32(self, _v: i32) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_i64(self, _v: i64) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_i128(self, _v: i128) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_u8(self, _v: u8) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_u16(self, _v: u16) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_u32(self, _v: u32) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_u64(self, _v: u64) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_u128(self, _v: u128) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_f32(self, _v: f32) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_f64(self, _v: f64) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_char(self, _v: char) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_str(self, value: &str) -> Result<()> {
        let RawValueStrEmitter(serializer) = self;
        serializer
            .formatter
            .write_raw_value(&mut serializer.writer, value)
            .map_err(Error::io)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_none(self) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_unit(self) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(ser::Error::custom("expected RawValue"))
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Display,
    {
        self.serialize_str(&value.to_string())
    }
}

/// Serialize the given data structure as JSON into the I/O stream.
///
/// Serialization guarantees it only feeds valid UTF-8 sequences to the writer.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: WriterExt,
    T: ?Sized + Serialize,
{
    let mut ser = Serializer::new(writer);
    value.serialize(&mut ser)
}

/// Serialize the given data structure as pretty-printed JSON into the I/O
/// stream.
///
/// Serialization guarantees it only feeds valid UTF-8 sequences to the writer.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_writer_pretty<W, T>(writer: W, value: &T) -> Result<()>
where
    W: WriterExt,
    T: ?Sized + Serialize,
{
    let mut ser = Serializer::pretty(writer);
    value.serialize(&mut ser)
}

/// Serialize the given data structure as a JSON byte vector.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    let mut writer = Vec::with_capacity(128);
    tri!(to_writer(&mut writer, value));
    Ok(writer)
}

/// Serialize the given data structure as a pretty-printed JSON byte vector.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_vec_pretty<T>(value: &T) -> Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    let mut writer = Vec::with_capacity(128);
    tri!(to_writer_pretty(&mut writer, value));
    Ok(writer)
}

/// Serialize the given data structure as a String of JSON.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_string<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let vec = tri!(to_vec(value));
    let string = unsafe {
        // We do not emit invalid UTF-8.
        String::from_utf8_unchecked(vec)
    };
    Ok(string)
}

/// Serialize the given data structure as a pretty-printed String of JSON.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_string_pretty<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let vec = tri!(to_vec_pretty(value));
    let string = unsafe {
        // We do not emit invalid UTF-8.
        String::from_utf8_unchecked(vec)
    };
    Ok(string)
}
