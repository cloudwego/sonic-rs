//! Formtter for JSON serialization.

// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.

use std::io::{self, Write};

use crate::{serde::tri, util::string::format_string, writer::WriteExt};

/// This trait abstracts away serializing the JSON control characters, which allows the user to
/// optionally pretty print the JSON output.
pub trait Formatter: Clone {
    /// Writes a `null` value to the specified writer.
    #[inline]
    fn write_null<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"null")
    }

    /// Writes a `true` or `false` value to the specified writer.
    #[inline]
    fn write_bool<W>(&mut self, writer: &mut W, value: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if value {
            writer.write_all(b"true")
        } else {
            writer.write_all(b"false")
        }
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i8<W>(&mut self, writer: &mut W, value: i8) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i16<W>(&mut self, writer: &mut W, value: i16) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i32<W>(&mut self, writer: &mut W, value: i32) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i128<W>(&mut self, writer: &mut W, value: i128) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u8<W>(&mut self, writer: &mut W, value: u8) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u16<W>(&mut self, writer: &mut W, value: u16) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u32<W>(&mut self, writer: &mut W, value: u32) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u64<W>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u128<W>(&mut self, writer: &mut W, value: u128) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);
        writer.write_all(s.as_bytes())
    }

    /// Writes a floating point value like `-31.26e+12` to the specified writer.
    #[inline]
    fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        #[cfg(feature = "non_trailing_zero")]
        {
            const F32_SAFE_GO_JSON_INT_MAX: f32 = {
                // This is the end of the contiguous prefix where formatting an
                // integral f32 through the integer fast path matches Go's
                // shortest-float JSON output. It is not the mathematical upper
                // bound of integers representable by f32.
                //
                // f32 has 24 bits of precision. In [2^25, 2^26), ULP is 4. The
                // next representable value after 2^25 + 12 is 33_554_448, whose
                // shortest round-tripping decimal is 33_554_450. The integer
                // fast path would instead print the exact f32 integer value,
                // 33_554_448, so stop at 2^25 + 12.
                const END_OF_CONTIGUOUS_SAFE_PREFIX: u32 = (1 << 25) + 12;
                assert!(
                    ((END_OF_CONTIGUOUS_SAFE_PREFIX + 1) as f32)
                        == (END_OF_CONTIGUOUS_SAFE_PREFIX as f32)
                );
                assert!(
                    ((END_OF_CONTIGUOUS_SAFE_PREFIX + 2) as f32)
                        == ((END_OF_CONTIGUOUS_SAFE_PREFIX + 4) as f32)
                );

                END_OF_CONTIGUOUS_SAFE_PREFIX as f32
            };

            if value != 0.0
                && value.fract() == 0.0
                && (-F32_SAFE_GO_JSON_INT_MAX..=F32_SAFE_GO_JSON_INT_MAX).contains(&value)
            {
                return self.write_i64(writer, value as i64);
            }
        }

        let mut buffer = zmij::Buffer::new();
        let s = buffer.format_finite(value);

        #[cfg(feature = "non_trailing_zero")]
        let s = if value.fract() == 0.0 {
            s.strip_suffix(".0").unwrap_or(s)
        } else {
            s
        };

        writer.write_all(s.as_bytes())
    }

    /// Writes a floating point value like `-31.26e+12` to the specified writer.
    #[inline]
    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        #[cfg(feature = "non_trailing_zero")]
        {
            const F64_SAFE_GO_JSON_INT_MAX: f64 = {
                // See the f32 boundary above. The same issue appears for f64
                // once ULP reaches 4 in [2^54, 2^55). The next representable
                // value after 2^54 + 4 is 2^54 + 8, whose shorter round-tripping
                // decimal can differ from the exact integer value printed by
                // the integer fast path.
                const END_OF_CONTIGUOUS_SAFE_PREFIX: u64 = (1 << 54) + 4;
                assert!(
                    ((END_OF_CONTIGUOUS_SAFE_PREFIX + 1) as f64)
                        == (END_OF_CONTIGUOUS_SAFE_PREFIX as f64)
                );
                assert!(
                    ((END_OF_CONTIGUOUS_SAFE_PREFIX + 2) as f64)
                        == ((END_OF_CONTIGUOUS_SAFE_PREFIX + 4) as f64)
                );

                END_OF_CONTIGUOUS_SAFE_PREFIX as f64
            };

            if value != 0.0
                && value.fract() == 0.0
                && (-F64_SAFE_GO_JSON_INT_MAX..=F64_SAFE_GO_JSON_INT_MAX).contains(&value)
            {
                return self.write_i64(writer, value as i64);
            }
        }

        let mut buffer = zmij::Buffer::new();
        let s = buffer.format_finite(value);

        #[cfg(feature = "non_trailing_zero")]
        let s = if value.fract() == 0.0 {
            s.strip_suffix(".0").unwrap_or(s)
        } else {
            s
        };

        writer.write_all(s.as_bytes())
    }

    /// Writes a number that has already been rendered to a string.
    #[inline]
    fn write_number_str<W>(&mut self, writer: &mut W, value: &str) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(value.as_bytes())
    }

    /// Writes a string as JSON string to the specified writer. Will escape the string if necessary.
    /// If `need_quote` is `false`, the string will be written without quotes.
    #[inline(always)]
    fn write_string_fast<W>(
        &mut self,
        writer: &mut W,
        value: &str,
        need_quote: bool,
    ) -> io::Result<()>
    where
        W: ?Sized + WriteExt,
    {
        let buf = writer.reserve_with(value.len() * 6 + 32 + 3)?;
        let cnt = format_string(value, buf, need_quote);
        unsafe { writer.flush_len(cnt)? };
        Ok(())
    }

    /// Writes the representation of a byte array. Formatters can choose whether
    /// to represent bytes as a JSON array of integers (the default), or some
    /// JSON string encoding like hex or base64.
    fn write_byte_array<W>(&mut self, writer: &mut W, value: &[u8]) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        tri!(self.begin_array(writer));
        let mut first = true;
        for byte in value {
            tri!(self.begin_array_value(writer, first));
            tri!(self.write_u8(writer, *byte));
            tri!(self.end_array_value(writer));
            first = false;
        }
        self.end_array(writer)
    }

    /// Only called before stringifing some values, such as integer key in HashMap
    #[inline]
    fn begin_string<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"\"")
    }

    /// Only called after stringifing some values, such as integer key in HashMap
    #[inline]
    fn end_string<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"\"")
    }

    /// Called before every array.  Writes a `[` to the specified
    /// writer.
    #[inline]
    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"[")
    }

    /// Called after every array.  Writes a `]` to the specified
    /// writer.
    #[inline]
    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"]")
    }

    /// Called before every array value.  Writes a `,` if needed to
    /// the specified writer.
    #[inline]
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b",")
        }
    }

    /// Called after every array value.
    #[inline]
    fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        Ok(())
    }

    /// Called before every object.  Writes a `{` to the specified
    /// writer.
    #[inline]
    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"{")
    }

    /// Called after every object.  Writes a `}` to the specified
    /// writer.
    #[inline]
    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b"}")
    }

    /// Called before every object key.
    #[inline]
    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b",")
        }
    }

    /// Called after every object key.  A `:` should be written to the
    /// specified writer by either this method or
    /// `begin_object_value`.
    #[inline]
    fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        Ok(())
    }

    /// Called before every object value.  A `:` should be written to
    /// the specified writer by either this method or
    /// `end_object_key`.
    #[inline]
    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b":")
    }

    /// Called after every object value.
    #[inline]
    fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        Ok(())
    }

    /// Writes a raw JSON value that doesn't need any escaping to the
    /// specified writer.
    #[inline]
    fn write_raw_value<W>(&mut self, writer: &mut W, raw: &str) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(raw.as_bytes())
    }
}

/// This structure compacts a JSON value with no extra whitespace.
#[derive(Clone, Debug)]
pub struct CompactFormatter;

impl Formatter for CompactFormatter {}

/// This structure pretty prints a JSON value to make it human readable.
#[derive(Clone, Debug)]
pub struct PrettyFormatter<'a> {
    current_indent: usize,
    has_value: bool,
    indent: &'a [u8],
}

impl<'a> PrettyFormatter<'a> {
    /// Construct a pretty printer formatter that defaults to using two spaces for indentation.
    pub fn new() -> Self {
        PrettyFormatter::with_indent(b"  ")
    }

    /// Construct a pretty printer formatter that uses the `indent` string for indentation.
    pub fn with_indent(indent: &'a [u8]) -> Self {
        PrettyFormatter {
            current_indent: 0,
            has_value: false,
            indent,
        }
    }
}

impl<'a> Default for PrettyFormatter<'a> {
    fn default() -> Self {
        PrettyFormatter::new()
    }
}

impl<'a> Formatter for PrettyFormatter<'a> {
    #[inline]
    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent += 1;
        self.has_value = false;
        writer.write_all(b"[")
    }

    #[inline]
    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent -= 1;

        if self.has_value {
            tri!(writer.write_all(b"\n"));
            tri!(indent(writer, self.current_indent, self.indent));
        }

        writer.write_all(b"]")
    }

    #[inline]
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        tri!(writer.write_all(if first { b"\n" } else { b",\n" }));
        indent(writer, self.current_indent, self.indent)
    }

    #[inline]
    fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.has_value = true;
        Ok(())
    }

    #[inline]
    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent += 1;
        self.has_value = false;
        writer.write_all(b"{")
    }

    #[inline]
    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent -= 1;

        if self.has_value {
            tri!(writer.write_all(b"\n"));
            tri!(indent(writer, self.current_indent, self.indent));
        }

        writer.write_all(b"}")
    }

    #[inline]
    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        tri!(writer.write_all(if first { b"\n" } else { b",\n" }));
        indent(writer, self.current_indent, self.indent)
    }

    #[inline]
    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b": ")
    }

    #[inline]
    fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.has_value = true;
        Ok(())
    }
}

fn indent<W>(wr: &mut W, n: usize, s: &[u8]) -> io::Result<()>
where
    W: ?Sized + Write,
{
    for _ in 0..n {
        tri!(wr.write_all(s));
    }

    Ok(())
}
