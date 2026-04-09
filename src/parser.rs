use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Debug,
    num::NonZeroU8,
    ops::Deref,
    slice::{from_raw_parts, from_raw_parts_mut},
    str::{from_utf8, from_utf8_unchecked},
};

use faststr::FastStr;
use serde::de::{self, Expected, Unexpected};
use sonic_number::{parse_number, ParserNumber};
#[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
use sonic_simd::bits::NeonBits;
use sonic_simd::{i8x32, m8x32, u8x32, u8x64, Mask, Simd};

use crate::{
    config::DeserializeCfg,
    error::{
        Error,
        ErrorCode::{self, *},
        Result,
    },
    index::Index,
    lazyvalue::value::HasEsc,
    pointer::{
        tree::{MultiIndex, MultiKey, PointerTreeInner, PointerTreeNode},
        PointerTree,
    },
    reader::Reader,
    serde::de::invalid_type_number,
    util::{
        arch::{get_nonspace_bits, prefix_xor},
        string::*,
        unicode::{codepoint_to_utf8, hex_to_u32_nocheck},
    },
    value::visitor::JsonVisitor,
    JsonValueMutTrait, JsonValueTrait, LazyValue, Number, OwnedLazyValue,
};

// support borrow for owned deserizlie or skip
pub enum Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    Borrowed(&'b T),
    Copied(&'c T),
}

impl<'b, 'c> From<Reference<'b, 'c, str>> for Cow<'b, str> {
    fn from(value: Reference<'b, 'c, str>) -> Self {
        match value {
            Reference::Borrowed(b) => Cow::Owned(b.to_string()),
            Reference::Copied(c) => Cow::Owned(c.to_string()),
        }
    }
}

impl<'b, 'c, T: Debug + ?Sized + 'static> Debug for Reference<'b, 'c, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Borrowed(c) => write!(f, "Borrowed({c:?})"),
            Self::Copied(c) => write!(f, "Copied({c:?})"),
        }
    }
}

impl<'b, 'c, T> Deref for Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Reference::Borrowed(b) => b,
            Reference::Copied(c) => c,
        }
    }
}

pub(crate) enum ParsedSlice<'b, 'c> {
    Borrowed {
        slice: &'b [u8],
        buf: &'c mut Vec<u8>,
    },
    Copied(&'c mut Vec<u8>),
}

impl<'b, 'c> Deref for ParsedSlice<'b, 'c> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            ParsedSlice::Borrowed { slice, buf: _ } => slice,
            ParsedSlice::Copied(c) => c.as_slice(),
        }
    }
}

pub(crate) const DEFAULT_KEY_BUF_CAPACITY: usize = 128;
pub(crate) fn as_str(data: &[u8]) -> &str {
    debug_assert!(from_utf8(data).is_ok(), "invalid utf-8 in as_str");
    unsafe { from_utf8_unchecked(data) }
}

macro_rules! impl_get_escaped_branchless {
    ($name:ident, $ty:ty, $even_bits:expr) => {
        #[inline(always)]
        fn $name(prev_escaped: &mut $ty, backslash: $ty) -> $ty {
            const EVEN_BITS: $ty = $even_bits;
            let backslash = backslash & (!*prev_escaped);
            let follows_escape = (backslash << 1) | *prev_escaped;
            let odd_sequence_starts = backslash & !EVEN_BITS & !follows_escape;
            let (sequences_starting_on_even_bits, overflow) =
                odd_sequence_starts.overflowing_add(backslash);
            *prev_escaped = overflow as $ty;
            let invert_mask = sequences_starting_on_even_bits << 1;
            (EVEN_BITS ^ invert_mask) & follows_escape
        }
    };
}

impl_get_escaped_branchless!(get_escaped_branchless_u32, u32, 0x5555_5555);
impl_get_escaped_branchless!(get_escaped_branchless_u64, u64, 0x5555_5555_5555_5555);

macro_rules! perr {
    ($self:ident, $err:expr) => {{
        Err($self.error($err))
    }};
}

macro_rules! check_visit {
    ($self:ident, $e:expr $(,)?) => {
        if !($e) {
            perr!($self, UnexpectedVisitType)
        } else {
            Ok(())
        }
    };
}

#[inline(always)]
pub(crate) fn is_whitespace(ch: u8) -> bool {
    // NOTE: the compiler not optimize as lookup, so we hard code here.
    const SPACE_MASK: u64 = (1u64 << b' ') | (1u64 << b'\r') | (1u64 << b'\n') | (1u64 << b'\t');
    1u64.checked_shl(ch as u32)
        .is_some_and(|v| v & SPACE_MASK != 0)
}

#[inline(always)]
fn get_string_bits(data: &[u8; 64], prev_instring: &mut u64, prev_escaped: &mut u64) -> u64 {
    let v = unsafe { u8x64::from_slice_unaligned_unchecked(data) };

    let bs_bits = (v.eq(&u8x64::splat(b'\\'))).bitmask();
    let escaped: u64;
    if bs_bits != 0 {
        escaped = get_escaped_branchless_u64(prev_escaped, bs_bits);
    } else {
        escaped = *prev_escaped;
        *prev_escaped = 0;
    }
    let quote_bits = (v.eq(&u8x64::splat(b'"'))).bitmask() & !escaped;
    let in_string = unsafe { prefix_xor(quote_bits) ^ *prev_instring };
    *prev_instring = (in_string as i64 >> 63) as u64;
    in_string
}

#[inline(always)]
fn skip_container_loop(
    input: &[u8; 64],        /* a 64-bytes slice from json */
    prev_instring: &mut u64, /* the bitmap of last string */
    prev_escaped: &mut u64,
    lbrace_num: &mut usize,
    rbrace_num: &mut usize,
    left: u8,
    right: u8,
) -> Option<NonZeroU8> {
    // get the bitmao
    let instring = get_string_bits(input, prev_instring, prev_escaped);
    // #Safety
    // the input is 64 bytes, so the v is always valid.
    let v = unsafe { u8x64::from_slice_unaligned_unchecked(input) };
    let last_lbrace_num = *lbrace_num;
    let mut rbrace = (v.eq(&u8x64::splat(right))).bitmask() & !instring;
    let lbrace = (v.eq(&u8x64::splat(left))).bitmask() & !instring;
    while rbrace != 0 {
        *rbrace_num += 1;
        *lbrace_num = last_lbrace_num + (lbrace & (rbrace - 1)).count_ones() as usize;
        let is_closed = lbrace_num < rbrace_num;
        if is_closed {
            debug_assert_eq!(*rbrace_num, *lbrace_num + 1);
            let cnt = rbrace.trailing_zeros() + 1;
            return unsafe { Some(NonZeroU8::new_unchecked(cnt as u8)) };
        }
        rbrace &= rbrace - 1;
    }
    *lbrace_num = last_lbrace_num + lbrace.count_ones() as usize;
    None
}

pub(crate) struct Pair<'de> {
    pub key: Cow<'de, str>,
    pub val: &'de [u8],
    pub status: ParseStatus,
}

pub struct Parser<R> {
    pub read: R,
    error_index: usize,   // mark the error position
    nospace_bits: u64,    // SIMD marked nospace bitmap
    nospace_start: isize, // the start position of nospace_bits
    pub(crate) cfg: DeserializeCfg,
}

/// Records the parse status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStatus {
    None,
    HasEscaped,
}

impl From<ParseStatus> for HasEsc {
    fn from(value: ParseStatus) -> Self {
        match value {
            ParseStatus::None => HasEsc::None,
            ParseStatus::HasEscaped => HasEsc::Yes,
        }
    }
}

impl<'de, R> Parser<R>
where
    R: Reader<'de>,
{
    pub fn new(read: R) -> Self {
        Self {
            read,
            error_index: usize::MAX,
            nospace_bits: 0,
            nospace_start: -128,
            cfg: DeserializeCfg::default(),
        }
    }

    pub fn offset(&self) -> usize {
        self.read.index()
    }

    /// Enable lossy UTF-8 handling: invalid surrogates produce U+FFFD replacement chars
    /// instead of errors. Matches Go's encoding/json behavior.
    pub fn utf8_lossy(mut self) -> Self {
        self.cfg.utf8_lossy = true;
        self
    }

    pub(crate) fn with_config(mut self, cfg: DeserializeCfg) -> Self {
        self.cfg = cfg;
        self
    }

    #[inline(always)]
    fn error_index(&self) -> usize {
        // when parsing strings , we need record the error position.
        // it must be smaller than reader.index().
        std::cmp::min(self.error_index, self.read.index().saturating_sub(1))
    }

    /// Error caused by a byte from next_char().
    #[cold]
    pub fn error(&self, mut reason: ErrorCode) -> Error {
        // check invalid utf8 here at first
        // FIXME: maybe has invalid utf8 when deserializing into byte, and just bytes has other
        // errors?
        if let Err(e) = self.read.check_utf8_final() {
            return e;
        }

        // check errors, if exceed, the reason must be eof, and begin parsing the padding chars
        let mut index = self.error_index();
        let len = self.read.as_u8_slice().len();
        if index > len {
            reason = EofWhileParsing;
            index = len;
        }
        Error::syntax(reason, self.read.origin_input(), index)
    }

    // maybe error in generated in visitor, so we need fix the position.
    #[cold]
    pub(crate) fn fix_position(&self, err: Error) -> Error {
        if err.line() == 0 {
            self.error(err.error_code())
        } else {
            err
        }
    }

    #[inline(always)]
    pub fn parse_number(&mut self, first: u8) -> Result<ParserNumber> {
        let reader = &mut self.read;
        let neg = first == b'-';
        let mut now = reader.index() - (!neg as usize);
        let data = reader.as_u8_slice();
        let ret = parse_number(data, &mut now, neg);
        reader.set_index(now);
        ret.map_err(|err| self.error(err.into()))
    }

    /// Parse a JSON string and visit it.
    /// When `strbuf` is Some, copies into the buffer (owned, calls visit_str).
    /// When `strbuf` is None, parses inplace zero-copy (calls visit_borrowed_str).
    #[inline(always)]
    fn parse_string_visit<V>(&mut self, vis: &mut V, strbuf: Option<&mut Vec<u8>>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        if let Some(strbuf) = strbuf {
            let rs = self.parse_str(strbuf)?;
            check_visit!(self, vis.visit_str(rs.as_ref()))
        } else {
            unsafe {
                let mut src = self.read.cur_ptr();
                let start = self.read.cur_ptr();
                let cnt = parse_string_inplace(&mut src, self.cfg.utf8_lossy)
                    .map_err(|e| self.error(e))?;
                self.read.set_ptr(src);
                let slice = from_raw_parts(start, cnt);
                let s = from_utf8_unchecked(slice);
                check_visit!(self, vis.visit_borrowed_str(s))
            }
        }
    }

    /// Fast path for keys that terminate within 24 bytes and contain no escapes (`\\`)
    /// or control characters (`< 0x20`). Accepts any valid UTF-8 bytes (including
    /// multi-byte sequences). Falls back to `parse_string_visit` on escape, control
    /// byte, or if no closing `"` is found within 24 bytes.
    ///
    /// # Safety
    /// Only called when strbuf=None (padded reader path). PaddedSliceRead has 64 bytes
    /// of zero-padding beyond valid JSON, so scanning 24 bytes ahead is always safe.
    /// Padding bytes (0x00) are < 0x20 and will bail to parse_string_visit.
    #[inline(always)]
    fn parse_key_scalar<V>(&mut self, vis: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        unsafe {
            let mut p = self.read.cur_ptr();
            let start = p;
            let end = p.add(24);
            while p < end {
                let ch = *p;
                if ch == b'"' {
                    let len = p.offset_from(start) as usize;
                    self.read.set_ptr(p.add(1));
                    let s = std::str::from_utf8_unchecked(std::slice::from_raw_parts(start, len));
                    return check_visit!(self, vis.visit_borrowed_str(s));
                }
                if ch == b'\\' || ch < 0x20 {
                    return self.parse_string_visit(vis, None);
                }
                p = p.add(1);
            }
            self.parse_string_visit(vis, None)
        }
    }

    /// Parse a number. When `inplace` is true, visits as borrowed raw number.
    #[inline(always)]
    fn parse_number_visit<V>(&mut self, first: u8, vis: &mut V, inplace: bool) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        if self.cfg.use_rawnumber {
            let start = self.read.index() - 1;
            self.skip_number(first)?;
            let slice = self.read.slice_unchecked(start, self.read.index());
            let ok = if inplace {
                vis.visit_borrowed_raw_number(as_str(slice))
            } else {
                vis.visit_raw_number(as_str(slice))
            };
            check_visit!(self, ok)
        } else {
            let ok = match self.parse_number(first)? {
                ParserNumber::Float(f) => vis.visit_f64(f),
                ParserNumber::Unsigned(f) => vis.visit_u64(f),
                ParserNumber::Signed(f) => vis.visit_i64(f),
            };
            check_visit!(self, ok)
        }
    }

    fn parse_array<V>(&mut self, vis: &mut V, mut strbuf: Option<&mut Vec<u8>>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        check_visit!(self, vis.visit_array_start(0))?;

        let mut first = match self.skip_space() {
            Some(b']') => return check_visit!(self, vis.visit_array_end(0)),
            first => first,
        };

        let mut count = 0;
        loop {
            self.dispatch_value(first, vis, &mut strbuf)?;
            count += 1;
            // Compact: u16 read for single-instruction matching
            let sep = self.read.peek_u16();
            if (sep & 0xFF) == b',' as u16 {
                let val_ch = (sep >> 8) as u8;
                if !is_whitespace(val_ch) {
                    self.read.eat(2);
                    first = Some(val_ch);
                    continue;
                }
            }
            if (sep & 0xFF) == b']' as u16 {
                self.read.eat(1);
                return check_visit!(self, vis.visit_array_end(count));
            }
            // Slow path
            first = match self.skip_space() {
                Some(b']') => return check_visit!(self, vis.visit_array_end(count)),
                Some(b',') => self.skip_space(),
                _ => return perr!(self, ExpectedArrayCommaOrEnd),
            };
        }
    }

    fn parse_object<V>(&mut self, vis: &mut V, mut strbuf: Option<&mut Vec<u8>>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let mut count: usize = 0;
        check_visit!(self, vis.visit_object_start(0))?;
        match self.skip_space() {
            Some(b'}') => return check_visit!(self, vis.visit_object_end(0)),
            Some(b'"') => {}
            _ => return perr!(self, ExpectObjectKeyOrEnd),
        }

        loop {
            // ---- parse key (scalar fast path for short ASCII keys) ----
            if strbuf.is_none() {
                self.parse_key_scalar(vis)?;
            } else {
                self.parse_string_visit(vis, strbuf.as_deref_mut())?;
            }

            // ---- find ':' + value start byte ----
            // Use u16 read: on little-endian, ':' followed by val_ch = (val_ch << 8) | ':'
            let pair = self.read.peek_u16();
            let next = if (pair & 0xFF) == b':' as u16 {
                let val_ch = (pair >> 8) as u8;
                if !is_whitespace(val_ch) {
                    self.read.eat(2);
                    Some(val_ch)
                } else {
                    self.parse_object_clo()?;
                    self.skip_space()
                }
            } else {
                self.parse_object_clo()?;
                self.skip_space()
            };

            // ---- parse value ----
            self.dispatch_value(next, vis, &mut strbuf)?;
            count += 1;

            // ---- find separator: one u16 read to match `,"` or `}x` ----
            let sep = self.read.peek_u16();
            // Little-endian: `,"` = 0x222C, `}x` = (x << 8) | 0x7D
            if sep == u16::from_le_bytes([b',', b'"']) {
                self.read.eat(2);
                continue;
            }
            if (sep & 0xFF) == b'}' as u16 {
                self.read.eat(1);
                return check_visit!(self, vis.visit_object_end(count));
            }
            // Slow path
            match self.skip_space() {
                Some(b'}') => return check_visit!(self, vis.visit_object_end(count)),
                Some(b',') => match self.skip_space() {
                    Some(b'"') => continue,
                    _ => return perr!(self, ExpectObjectKeyOrEnd),
                },
                _ => return perr!(self, ExpectedArrayCommaOrEnd),
            }
        }
    }

    /// Dispatch value parsing based on the peeked byte.
    /// When `strbuf` is None, strings are parsed inplace (zero-copy borrowed).
    /// When `strbuf` is Some, strings are parsed into the buffer (owned copy).
    #[inline(always)]
    fn dispatch_value<V>(
        &mut self,
        ch: Option<u8>,
        vis: &mut V,
        strbuf: &mut Option<&mut Vec<u8>>,
    ) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        match ch {
            Some(c @ b'-' | c @ b'0'..=b'9') => self.parse_number_visit(c, vis, strbuf.is_none()),
            Some(b'"') => self.parse_string_visit(vis, strbuf.as_deref_mut()),
            Some(b'{') => self.parse_object(vis, strbuf.as_deref_mut()),
            Some(b'[') => self.parse_array(vis, strbuf.as_deref_mut()),
            Some(first) => self.parse_literal_visit(first, vis),
            None => perr!(self, EofWhileParsing),
        }
    }

    #[inline(always)]
    fn parse_literal_visit<V>(&mut self, first: u8, vis: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let literal = match first {
            b't' => "rue",
            b'f' => "alse",
            b'n' => "ull",
            _ => return perr!(self, InvalidJsonValue),
        };

        let reader = &mut self.read;
        if let Some(chunk) = reader.next_n(literal.len()) {
            if chunk != literal.as_bytes() {
                return perr!(self, InvalidLiteral);
            }

            let ok = match first {
                b't' => vis.visit_bool(true),
                b'f' => vis.visit_bool(false),
                b'n' => vis.visit_null(),
                _ => unreachable!(),
            };
            check_visit!(self, ok)
        } else {
            perr!(self, EofWhileParsing)
        }
    }

    #[inline]
    pub(crate) fn parse_array_elem_lazy(
        &mut self,
        first: &mut bool,
        check: bool,
    ) -> Result<Option<(&'de [u8], ParseStatus)>> {
        if *first && self.skip_space() != Some(b'[') {
            return perr!(self, ExpectedArrayStart);
        }
        match self.skip_space_peek() {
            Some(b']') => {
                self.read.eat(1);
                return Ok(None);
            }
            Some(b',') if !(*first) => {
                self.read.eat(1);
            }
            Some(_) if *first => {
                *first = false;
            }
            _ => return perr!(self, ExpectedArrayCommaOrEnd),
        };
        let (raw, status) = self.skip_one(check)?;
        Ok(Some((raw, status)))
    }

    #[inline]
    pub(crate) fn parse_entry_lazy(
        &mut self,
        strbuf: &mut Vec<u8>,
        first: &mut bool,
        check: bool,
    ) -> Result<Option<Pair<'de>>> {
        if *first && self.skip_space() != Some(b'{') {
            return perr!(self, ExpectedObjectStart);
        }
        match self.skip_space() {
            Some(b'}') => return Ok(None),
            Some(b'"') if *first => *first = false,
            Some(b',') if !*first => {
                if self.skip_space() != Some(b'"') {
                    return perr!(self, ExpectObjectKeyOrEnd);
                }
            }
            _ => return perr!(self, ExpectedObjectCommaOrEnd),
        }

        let parsed = self.parse_str(strbuf)?;
        self.parse_object_clo()?;
        let (raw, status) = self.skip_one(check)?;

        Ok(Some(Pair {
            key: parsed.into(),
            val: raw,
            status,
        }))
    }

    #[inline(always)]
    pub(crate) fn match_literal(&mut self, literal: &'static str) -> Result<bool> {
        if let Some(chunk) = self.read.next_n(literal.len()) {
            if chunk != literal.as_bytes() {
                perr!(self, InvalidLiteral)
            } else {
                Ok(true)
            }
        } else {
            perr!(self, EofWhileParsing)
        }
    }

    #[inline(always)]
    pub(crate) fn get_owned_lazyvalue(&mut self, strict: bool) -> Result<OwnedLazyValue> {
        let c = self.skip_space();
        let start = match c {
            Some(b'"') => {
                let start = self.read.index() - 1;
                match self.skip_string()? {
                    ParseStatus::None => {
                        let slice = self.read.slice_unchecked(start, self.read.index());
                        let raw = self.read.slice_ref(slice).as_faststr();
                        return Ok(OwnedLazyValue::from_non_esc_str(raw));
                    }
                    ParseStatus::HasEscaped => {}
                }
                start
            }
            Some(b't') if self.match_literal("rue")? => return Ok(true.into()),
            Some(b'f') if self.match_literal("alse")? => return Ok(false.into()),
            Some(b'n') if self.match_literal("ull")? => return Ok(().into()),
            None => return perr!(self, EofWhileParsing),
            _ => {
                let start = self.read.index() - 1;
                self.read.backward(1);
                self.skip_one(strict)?;
                start
            }
        };
        let end = self.read.index();
        let sub = self.read.slice_unchecked(start, end);
        let raw = self.read.slice_ref(sub).as_faststr();
        Ok(OwnedLazyValue::new(raw.into(), HasEsc::Possible))
    }

    #[inline(always)]
    fn parse_faststr(&mut self, strbuf: &mut Vec<u8>) -> Result<FastStr> {
        match self.parse_str(strbuf)? {
            Reference::Borrowed(s) => {
                return Ok(self.read.slice_ref(s.as_bytes()).as_faststr());
            }
            Reference::Copied(s) => Ok(FastStr::new(s)),
        }
    }

    #[inline(always)]
    pub(crate) fn load_owned_lazyvalue(&mut self, strbuf: &mut Vec<u8>) -> Result<OwnedLazyValue> {
        match self.skip_space() {
            Some(c @ b'-' | c @ b'0'..=b'9') => {
                let num: Number = self.parse_number(c)?.into();
                Ok(OwnedLazyValue::from(num))
            }
            Some(b'"') => match self.parse_str(strbuf)? {
                Reference::Borrowed(s) => {
                    let raw = self.read.slice_ref(s.as_bytes()).as_faststr();
                    Ok(OwnedLazyValue::from_faststr(raw))
                }
                Reference::Copied(s) => {
                    let raw = FastStr::new(s);
                    Ok(OwnedLazyValue::from_faststr(raw))
                }
            },
            Some(b'{') => {
                // parsing empty object
                match self.skip_space() {
                    Some(b'}') => return Ok(Vec::<(FastStr, OwnedLazyValue)>::new().into()),
                    Some(b'"') => {}
                    _ => return perr!(self, ExpectObjectKeyOrEnd),
                }

                // loop for each object key and value
                let mut vec = Vec::with_capacity(32);
                loop {
                    let key = self.parse_faststr(strbuf)?;
                    self.parse_object_clo()?;
                    let olv = self.get_owned_lazyvalue(false)?;
                    vec.push((key, olv));
                    match self.skip_space() {
                        Some(b'}') => return Ok(vec.into()),
                        Some(b',') => match self.skip_space() {
                            Some(b'"') => continue,
                            _ => return perr!(self, ExpectObjectKeyOrEnd),
                        },
                        _ => return perr!(self, ExpectedArrayCommaOrEnd),
                    }
                }
            }
            Some(b'[') => {
                if let Some(b']') = self.skip_space() {
                    return Ok(Vec::<OwnedLazyValue>::new().into());
                }

                let mut vec = Vec::with_capacity(32);
                self.read.backward(1);
                loop {
                    vec.push(self.get_owned_lazyvalue(false)?);
                    match self.skip_space() {
                        Some(b']') => return Ok(vec.into()),
                        Some(b',') => {}
                        _ => return perr!(self, ExpectedArrayCommaOrEnd),
                    };
                }
            }
            _ => perr!(self, InvalidJsonValue),
        }
    }

    #[inline(always)]
    pub(crate) fn parse_dom<V>(
        &mut self,
        vis: &mut V,
        mut strbuf: Option<&mut Vec<u8>>,
    ) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        check_visit!(self, vis.visit_dom_start())?;
        let ch = self.skip_space();
        self.dispatch_value(ch, vis, &mut strbuf)?;
        check_visit!(self, vis.visit_dom_end())
    }

    #[inline(always)]
    pub fn parse_str<'own>(&mut self, buf: &'own mut Vec<u8>) -> Result<Reference<'de, 'own, str>> {
        match self.parse_string_raw(buf) {
            Ok(ParsedSlice::Copied(buf)) => {
                if self.check_invalid_utf8(self.cfg.utf8_lossy)? {
                    // repr the invalid utf-8
                    let repr = String::from_utf8_lossy(buf.as_ref()).into_owned();
                    *buf = repr.into_bytes();
                }
                let slice = unsafe { from_utf8_unchecked(buf.as_slice()) };
                Ok(Reference::Copied(slice))
            }
            Ok(ParsedSlice::Borrowed { slice, buf }) => {
                if self.check_invalid_utf8(self.cfg.utf8_lossy)? {
                    // repr the invalid utf-8
                    let repr = String::from_utf8_lossy(slice).into_owned();
                    *buf = repr.into_bytes();
                    let slice = unsafe { from_utf8_unchecked(buf) };
                    Ok(Reference::Copied(slice))
                } else {
                    Ok(Reference::Borrowed(unsafe { from_utf8_unchecked(slice) }))
                }
            }
            Err(e) => Err(e),
        }
    }

    pub(crate) fn check_invalid_utf8(&mut self, allowed: bool) -> Result<bool> {
        // the invalid UTF-8 before the string, must have been checked before.
        let invalid = self.read.next_invalid_utf8();
        if invalid >= self.read.index() {
            return Ok(false);
        }

        if !allowed {
            Err(Error::syntax(
                ErrorCode::InvalidUTF8,
                self.read.origin_input(),
                invalid,
            ))
        } else {
            // this space is allowed, should update the next invalid utf8 position
            self.read.check_invalid_utf8();
            Ok(true)
        }
    }

    pub(crate) fn parse_escaped_utf8(&mut self) -> Result<u32> {
        let point1 = if let Some(asc) = self.read.next_n(4) {
            unsafe { hex_to_u32_nocheck(&*(asc.as_ptr() as *const _ as *const [u8; 4])) }
        } else {
            return perr!(self, EofWhileParsing);
        };

        // only check surrogate here, and we will check the code pointer later when use
        // `codepoint_to_utf8`
        if (0xD800..0xDC00).contains(&point1) {
            // parse the second utf8 code point of surrogate
            let point2 = if let Some(asc) = self.read.next_n(6) {
                if asc[0] != b'\\' || asc[1] != b'u' {
                    if self.cfg.utf8_lossy {
                        // Backtrack so the non-\uXXXX bytes can be re-parsed
                        let idx = self.read.index();
                        self.read.set_index(idx - 6);
                        return Ok(0xFFFD);
                    } else {
                        return perr!(self, InvalidSurrogateUnicodeCodePoint);
                    }
                }
                unsafe { hex_to_u32_nocheck(&*(asc.as_ptr().add(2) as *const _ as *const [u8; 4])) }
            } else if self.cfg.utf8_lossy {
                return Ok(0xFFFD);
            } else {
                // invalid surrogate
                return perr!(self, InvalidSurrogateUnicodeCodePoint);
            };

            /* calcute the real code point */
            let low_bit = point2.wrapping_sub(0xdc00);
            if (low_bit >> 10) != 0 {
                if self.cfg.utf8_lossy {
                    // point2 is not a valid low surrogate. Backtrack 6 bytes
                    // so it can be re-parsed (e.g. \uDA51\uD83D\uDE04 → FFFD + 😄).
                    let idx = self.read.index();
                    self.read.set_index(idx - 6);
                    return Ok(0xFFFD);
                } else {
                    return perr!(self, InvalidSurrogateUnicodeCodePoint);
                }
            }

            Ok((((point1 - 0xd800) << 10) | low_bit).wrapping_add(0x10000))
        } else if (0xDC00..0xE000).contains(&point1) {
            if self.cfg.utf8_lossy {
                Ok(0xFFFD)
            } else {
                // invalid surrogate
                perr!(self, InvalidSurrogateUnicodeCodePoint)
            }
        } else {
            Ok(point1)
        }
    }

    pub(crate) unsafe fn parse_escaped_char(&mut self, buf: &mut Vec<u8>) -> Result<()> {
        'escape: loop {
            match self.read.next() {
                Some(b'u') => {
                    let code = self.parse_escaped_utf8()?;
                    buf.reserve(4);
                    let ptr = buf.as_mut_ptr().add(buf.len());
                    let cnt = codepoint_to_utf8(code, ptr);
                    if cnt == 0 {
                        return perr!(self, InvalidUnicodeCodePoint);
                    }
                    buf.set_len(buf.len() + cnt);
                }
                Some(c) if ESCAPED_TAB[c as usize] != 0 => {
                    buf.push(ESCAPED_TAB[c as usize]);
                }
                None => return perr!(self, EofWhileParsing),
                _ => return perr!(self, InvalidEscape),
            }

            // fast path for continuous escaped chars
            if self.read.peek() == Some(b'\\') {
                self.read.eat(1);
                continue 'escape;
            }
            break 'escape;
        }
        Ok(())
    }

    pub(crate) unsafe fn parse_string_escaped<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<ParsedSlice<'de, 'own>> {
        #[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
        let mut block: StringBlock<NeonBits>;
        #[cfg(not(all(target_feature = "neon", target_arch = "aarch64")))]
        let mut block: StringBlock<u32>;

        self.parse_escaped_char(buf)?;

        while let Some(chunk) = self.read.peek_n(StringBlock::LANES) {
            buf.reserve(StringBlock::LANES);
            let v = unsafe { load(chunk.as_ptr()) };
            block = StringBlock::new(&v);

            if block.has_unescaped() {
                self.read.eat(block.unescaped_index());
                return perr!(self, ControlCharacterWhileParsingString);
            }

            // write the chunk to buf, we will set new_len later
            let chunk = from_raw_parts_mut(buf.as_mut_ptr().add(buf.len()), StringBlock::LANES);
            v.write_to_slice_unaligned_unchecked(chunk);

            if block.has_quote_first() {
                let cnt = block.quote_index();
                buf.set_len(buf.len() + cnt);

                // skip the right quote
                self.read.eat(cnt + 1);
                return Ok(ParsedSlice::Copied(buf));
            }

            if block.has_backslash() {
                // TODO: loop unrooling here
                let cnt = block.bs_index();
                // skip the backslash
                self.read.eat(cnt + 1);
                buf.set_len(buf.len() + cnt);
                self.parse_escaped_char(buf)?;
            } else {
                buf.set_len(buf.len() + StringBlock::LANES);
                self.read.eat(StringBlock::LANES);
            }
        }

        // scalar codes
        while let Some(c) = self.read.peek() {
            match c {
                b'"' => {
                    self.read.eat(1);
                    return Ok(ParsedSlice::Copied(buf));
                }
                b'\\' => {
                    // skip the backslash
                    self.read.eat(1);
                    self.parse_escaped_char(buf)?;
                }
                b'\x00'..=b'\x1f' => return perr!(self, ControlCharacterWhileParsingString),
                _ => {
                    buf.push(c);
                    self.read.eat(1);
                }
            }
        }

        perr!(self, EofWhileParsing)
    }

    #[inline(always)]
    // parse_string_raw maybe borrowed, maybe copied into buf(buf will be clear at first).
    pub(crate) fn parse_string_raw<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<ParsedSlice<'de, 'own>> {
        // now reader is start after `"`, so we can directly skipstring
        let start = self.read.index();
        #[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
        let mut block: StringBlock<NeonBits>;
        #[cfg(not(all(target_feature = "neon", target_arch = "aarch64")))]
        let mut block: StringBlock<u32>;

        while let Some(chunk) = self.read.peek_n(StringBlock::LANES) {
            let v = unsafe { load(chunk.as_ptr()) };
            block = StringBlock::new(&v);

            if block.has_quote_first() {
                let cnt = block.quote_index();
                self.read.eat(cnt + 1);
                let slice = self.read.slice_unchecked(start, self.read.index() - 1);
                return Ok(ParsedSlice::Borrowed { slice, buf });
            }

            if block.has_unescaped() {
                self.read.eat(block.unescaped_index());
                return perr!(self, ControlCharacterWhileParsingString);
            }

            if block.has_backslash() {
                let cnt = block.bs_index();
                // skip the backslash
                self.read.eat(cnt + 1);

                // copy unescaped parts to buf
                buf.clear();
                buf.extend_from_slice(&self.read.as_u8_slice()[start..self.read.index() - 1]);

                return unsafe { self.parse_string_escaped(buf) };
            }

            self.read.eat(StringBlock::LANES);
            continue;
        }

        // found quote for remaining bytes
        while let Some(c) = self.read.peek() {
            match c {
                b'"' => {
                    self.read.eat(1);
                    let slice = self.read.slice_unchecked(start, self.read.index() - 1);
                    return Ok(ParsedSlice::Borrowed { slice, buf });
                }
                b'\\' => {
                    buf.clear();
                    buf.extend_from_slice(self.read.slice_unchecked(start, self.read.index()));
                    self.read.eat(1);
                    return unsafe { self.parse_string_escaped(buf) };
                }
                b'\x00'..=b'\x1f' => return perr!(self, ControlCharacterWhileParsingString),
                _ => self.read.eat(1),
            }
        }
        perr!(self, EofWhileParsing)
    }

    #[inline(always)]
    fn get_next_token<const N: usize>(&mut self, tokens: [u8; N], advance: usize) -> Option<u8> {
        let r = &mut self.read;
        const LANS: usize = u8x32::LANES;
        while let Some(chunk) = r.peek_n(LANS) {
            let v = unsafe { u8x32::from_slice_unaligned_unchecked(chunk) };
            let mut vor = m8x32::splat(false);
            for t in tokens.iter().take(N) {
                vor |= v.eq(&u8x32::splat(*t));
            }
            let next = vor.bitmask();
            if next != 0 {
                let cnt = next.trailing_zeros() as usize;
                let ch = chunk[cnt];
                r.eat(cnt + advance);
                return Some(ch);
            }
            r.eat(LANS);
        }

        while let Some(ch) = r.peek() {
            for t in tokens.iter().take(N) {
                if ch == *t {
                    r.eat(advance);
                    return Some(ch);
                }
            }
            r.eat(1)
        }
        None
    }

    // skip_string skips a JSON string, and return the later parts after closed quote, and the
    // escaped status. skip_string always start with the quote marks.
    #[inline(always)]
    unsafe fn skip_string_unchecked(&mut self) -> Result<ParseStatus> {
        const LANS: usize = u8x32::LANES;
        let r = &mut self.read;
        let mut quote_bits;
        let mut escaped;
        let mut prev_escaped = 0;
        let mut status = ParseStatus::None;

        while let Some(chunk) = r.peek_n(LANS) {
            let v = unsafe { u8x32::from_slice_unaligned_unchecked(chunk) };
            let bs_bits = (v.eq(&u8x32::splat(b'\\'))).bitmask();
            quote_bits = (v.eq(&u8x32::splat(b'"'))).bitmask();
            // maybe has escaped quotes
            if ((quote_bits.wrapping_sub(1)) & bs_bits) != 0 || prev_escaped != 0 {
                escaped = get_escaped_branchless_u32(&mut prev_escaped, bs_bits);
                status = ParseStatus::HasEscaped;
                quote_bits &= !escaped;
            }
            // real quote bits
            if quote_bits != 0 {
                // eat the ending quote mark
                r.eat(quote_bits.trailing_zeros() as usize + 1);
                return Ok(status);
            }
            r.eat(LANS)
        }

        // skip the possible prev escaped quote
        if prev_escaped != 0 {
            r.eat(1)
        }

        // found quote for remaining bytes
        while let Some(ch) = r.peek() {
            if ch == b'\\' {
                if r.remain() < 2 {
                    break;
                }
                status = ParseStatus::HasEscaped;
                r.eat(2);
                continue;
            }
            r.eat(1);
            if ch == b'"' {
                return Ok(status);
            }
        }
        perr!(self, EofWhileParsing)
    }

    fn skip_escaped_chars(&mut self) -> Result<()> {
        match self.read.peek() {
            Some(b'u') => {
                if self.read.remain() < 6 {
                    return perr!(self, EofWhileParsing);
                } else {
                    self.read.eat(5);
                }
            }
            Some(c) => {
                if self.read.next().is_none() {
                    return perr!(self, EofWhileParsing);
                }
                if ESCAPED_TAB[c as usize] == 0 {
                    return perr!(self, InvalidEscape);
                }
            }
            None => return perr!(self, EofWhileParsing),
        }
        Ok(())
    }

    // skip_string skips a JSON string with validation.
    #[inline(always)]
    fn skip_string(&mut self) -> Result<ParseStatus> {
        const LANS: usize = u8x32::LANES;

        let mut status = ParseStatus::None;
        while let Some(chunk) = self.read.peek_n(LANS) {
            let v = unsafe { u8x32::from_slice_unaligned_unchecked(chunk) };
            let v_bs = v.eq(&u8x32::splat(b'\\'));
            let v_quote = v.eq(&u8x32::splat(b'"'));
            let v_cc = v.le(&u8x32::splat(0x1f));
            let mask = (v_bs | v_quote | v_cc).bitmask();

            // check the mask
            if mask != 0 {
                let cnt = mask.trailing_zeros() as usize;
                self.read.eat(cnt + 1);

                match chunk[cnt] {
                    b'\\' => {
                        self.skip_escaped_chars()?;
                        status = ParseStatus::HasEscaped;
                    }
                    b'\"' => return Ok(status),
                    0..=0x1f => return perr!(self, ControlCharacterWhileParsingString),
                    _ => unreachable!(),
                }
            } else {
                self.read.eat(LANS)
            }
        }

        // found quote for remaining bytes
        while let Some(ch) = self.read.next() {
            match ch {
                b'\\' => {
                    self.skip_escaped_chars()?;
                    status = ParseStatus::HasEscaped;
                }
                b'"' => return Ok(status),
                0..=0x1f => return perr!(self, ControlCharacterWhileParsingString),
                _ => {}
            }
        }
        perr!(self, EofWhileParsing)
    }

    // parse the Colon :
    #[inline(always)]
    pub(crate) fn parse_object_clo(&mut self) -> Result<()> {
        if let Some(ch) = self.read.peek() {
            // fast path for compact json
            if ch == b':' {
                self.read.eat(1);
                return Ok(());
            }

            match self.skip_space() {
                Some(b':') => Ok(()),
                Some(_) => perr!(self, ExpectedColon),
                None => perr!(self, EofWhileParsing),
            }
        } else {
            perr!(self, EofWhileParsing)
        }
    }

    // parse the Colon :
    #[inline(always)]
    pub(crate) fn parse_array_end(&mut self) -> Result<()> {
        match self.skip_space() {
            Some(b']') => Ok(()),
            Some(_) => perr!(self, ExpectedArrayCommaOrEnd),
            None => perr!(self, EofWhileParsing),
        }
    }

    #[inline(always)]
    fn skip_object(&mut self) -> Result<()> {
        match self.skip_space() {
            Some(b'}') => return Ok(()),
            Some(b'"') => {}
            None => return perr!(self, EofWhileParsing),
            Some(_) => return perr!(self, ExpectObjectKeyOrEnd),
        }

        loop {
            self.skip_string()?;
            self.parse_object_clo()?;
            self.skip_one(true)?;

            match self.skip_space() {
                Some(b'}') => return Ok(()),
                Some(b',') => match self.skip_space() {
                    Some(b'"') => continue,
                    _ => return perr!(self, ExpectObjectKeyOrEnd),
                },
                None => return perr!(self, EofWhileParsing),
                Some(_) => return perr!(self, ExpectedObjectCommaOrEnd),
            }
        }
    }

    #[inline(always)]
    fn skip_array(&mut self) -> Result<()> {
        match self.skip_space_peek() {
            Some(b']') => {
                self.read.eat(1);
                return Ok(());
            }
            None => return perr!(self, EofWhileParsing),
            _ => {}
        }

        loop {
            self.skip_one(true)?;
            match self.skip_space() {
                Some(b']') => return Ok(()),
                Some(b',') => continue,
                None => return perr!(self, EofWhileParsing),
                _ => return perr!(self, ExpectedArrayCommaOrEnd),
            }
        }
    }

    /// skip_container skip a object or array, and retu
    #[inline(always)]
    fn skip_container(&mut self, left: u8, right: u8) -> Result<()> {
        let mut prev_instring = 0;
        let mut prev_escaped = 0;
        let mut rbrace_num = 0;
        let mut lbrace_num = 0;
        let reader = &mut self.read;

        while let Some(chunk) = reader.peek_n(64) {
            let input = unsafe { &*(chunk.as_ptr() as *const [_; 64]) };
            if let Some(count) = skip_container_loop(
                input,
                &mut prev_instring,
                &mut prev_escaped,
                &mut lbrace_num,
                &mut rbrace_num,
                left,
                right,
            ) {
                reader.eat(count.get() as usize);
                return Ok(());
            }
            reader.eat(64);
        }

        let mut remain = [0u8; 64];
        {
            let n = reader.remain();
            debug_assert!(n <= 64);
            remain[..n].copy_from_slice(reader.peek_n(n).unwrap());
        }
        if let Some(count) = skip_container_loop(
            &remain,
            &mut prev_instring,
            &mut prev_escaped,
            &mut lbrace_num,
            &mut rbrace_num,
            left,
            right,
        ) {
            reader.eat(count.get() as usize);
            return Ok(());
        }

        perr!(self, EofWhileParsing)
    }

    #[inline(always)]
    pub fn skip_space(&mut self) -> Option<u8> {
        let reader = &mut self.read;
        // fast path 1: for nospace or single space
        // most JSON is like ` "name": "balabala" `
        if let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return Some(ch);
            }
        }
        if let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return Some(ch);
            }
        }

        // fast path 2: reuse the bitmap for short key or numbers
        let nospace_offset = (reader.index() as isize) - self.nospace_start;
        if nospace_offset < 64 {
            let bitmap = {
                let mask = !((1 << nospace_offset) - 1);
                self.nospace_bits & mask
            };
            if bitmap != 0 {
                let cnt = bitmap.trailing_zeros() as usize;
                let ch = reader.at(self.nospace_start as usize + cnt);
                reader.set_index(self.nospace_start as usize + cnt + 1);

                return Some(ch);
            } else {
                // we can still fast skip the marked space in here.
                reader.set_index(self.nospace_start as usize + 64);
            }
        }

        // then we use simd to accelerate skipping space
        while let Some(chunk) = reader.peek_n(64) {
            let chunk = unsafe { &*(chunk.as_ptr() as *const [_; 64]) };
            let bitmap = unsafe { get_nonspace_bits(chunk) };
            if bitmap != 0 {
                self.nospace_bits = bitmap;
                self.nospace_start = reader.index() as isize;
                let cnt = bitmap.trailing_zeros() as usize;
                let ch = chunk[cnt];
                reader.eat(cnt + 1);

                return Some(ch);
            }
            reader.eat(64)
        }

        while let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                //
                return Some(ch);
            }
        }
        None
    }

    #[inline(always)]
    pub fn skip_space_peek(&mut self) -> Option<u8> {
        let ret = self.skip_space()?;
        self.read.backward(1);
        Some(ret)
    }

    #[inline(always)]
    pub fn parse_literal(&mut self, literal: &str) -> Result<()> {
        let reader = &mut self.read;
        if let Some(chunk) = reader.next_n(literal.len()) {
            if chunk == literal.as_bytes() {
                Ok(())
            } else {
                perr!(self, InvalidLiteral)
            }
        } else {
            perr!(self, EofWhileParsing)
        }
    }

    #[inline(always)]
    fn skip_number_unsafe(&mut self) -> Result<()> {
        let _ = self.get_next_token([b']', b'}', b','], 0);
        Ok(())
    }

    #[inline(always)]
    fn skip_exponent(&mut self) -> Result<()> {
        if let Some(ch) = self.read.peek() {
            if ch == b'-' || ch == b'+' {
                self.read.eat(1);
            }
        }
        self.skip_single_digit()?;
        // skip the remaining digits
        while matches!(self.read.peek(), Some(b'0'..=b'9')) {
            self.read.eat(1);
        }
        Ok(())
    }

    #[inline(always)]
    fn skip_single_digit(&mut self) -> Result<u8> {
        if let Some(ch) = self.read.next() {
            if !ch.is_ascii_digit() {
                perr!(self, InvalidNumber)
            } else {
                Ok(ch)
            }
        } else {
            perr!(self, EofWhileParsing)
        }
    }

    #[inline(always)]
    pub fn skip_number(&mut self, first: u8) -> Result<&'de str> {
        let start = self.read.index() - 1;
        self.do_skip_number(first)?;
        let end = self.read.index();
        Ok(as_str(self.read.slice_unchecked(start, end)))
    }

    #[inline(always)]
    pub(crate) fn do_skip_number(&mut self, mut first: u8) -> Result<()> {
        // check eof after the sign
        if first == b'-' {
            first = self.skip_single_digit()?;
        }

        // check the leading zeros
        let second = self.read.peek();
        if first == b'0' && matches!(second, Some(b'0'..=b'9')) {
            return perr!(self, InvalidNumber);
        }

        // fast path for the single digit
        let mut is_float: bool = false;
        match second {
            Some(b'0'..=b'9') => self.read.eat(1),
            Some(b'.') => {
                is_float = true;
                self.read.eat(1);
                self.skip_single_digit()?;
            }
            Some(b'e' | b'E') => {
                self.read.eat(1);
                return self.skip_exponent();
            }
            _ => return Ok(()),
        }

        // SIMD path for long number
        const LANES: usize = i8x32::LANES;
        while let Some(chunk) = self.read.peek_n(LANES) {
            let v = unsafe { i8x32::from_slice_unaligned_unchecked(chunk) };
            let zero = i8x32::splat(b'0' as i8);
            let nine = i8x32::splat(b'9' as i8);
            let mut nondigits = (zero.gt(&v) | v.gt(&nine)).bitmask();
            if nondigits != 0 {
                let mut cnt = nondigits.trailing_zeros() as usize;
                let ch = chunk[cnt];
                if ch == b'.' && !is_float {
                    self.read.eat(cnt + 1);
                    // check the first digit after the dot
                    self.skip_single_digit()?;

                    // check the overflow
                    cnt += 2;
                    if cnt >= LANES {
                        is_float = true;
                        continue;
                    }

                    nondigits = nondigits.wrapping_shr(cnt as u32);
                    if nondigits != 0 {
                        let offset = nondigits.trailing_zeros() as usize;
                        let ch = chunk[cnt + offset];
                        if ch == b'e' || ch == b'E' {
                            self.read.eat(offset + 1);
                            return self.skip_exponent();
                        } else {
                            self.read.eat(offset);
                            return Ok(());
                        }
                    } else {
                        self.read.eat(32 - cnt);
                        is_float = true;
                        continue;
                    }
                } else if ch == b'e' || ch == b'E' {
                    self.read.eat(cnt + 1);
                    return self.skip_exponent();
                } else {
                    self.read.eat(cnt);
                    return Ok(());
                }
            }
            // long digits
            self.read.eat(32);
        }

        // has less than 32 bytes
        while matches!(self.read.peek(), Some(b'0'..=b'9')) {
            self.read.eat(1);
        }

        match self.read.peek() {
            Some(b'.') if !is_float => {
                self.read.eat(1);
                self.skip_single_digit()?;
                while matches!(self.read.peek(), Some(b'0'..=b'9')) {
                    self.read.eat(1);
                }
                match self.read.peek() {
                    Some(b'e' | b'E') => {
                        self.read.eat(1);
                        return self.skip_exponent();
                    }
                    _ => return Ok(()),
                }
            }
            Some(b'e' | b'E') => {
                self.read.eat(1);
                return self.skip_exponent();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn skip_one(&mut self, checked: bool) -> Result<(&'de [u8], ParseStatus)> {
        let ch = match self.skip_space() {
            Some(ch) => ch,
            None => return perr!(self, EofWhileParsing),
        };
        let start = self.read.index() - 1;
        let mut status = ParseStatus::None;
        match ch {
            c @ b'-' | c @ b'0'..=b'9' => {
                if checked {
                    self.skip_number(c)?;
                } else {
                    self.skip_number_unsafe()?;
                }
                Ok(())
            }
            b'"' => {
                status = if checked {
                    self.skip_string()?
                } else {
                    unsafe { self.skip_string_unchecked() }?
                };
                Ok(())
            }
            b'{' => {
                if checked {
                    self.skip_object()
                } else {
                    self.skip_container(b'{', b'}')
                }
            }
            b'[' => {
                if checked {
                    self.skip_array()
                } else {
                    self.skip_container(b'[', b']')
                }
            }
            b't' => self.parse_literal("rue"),
            b'f' => self.parse_literal("alse"),
            b'n' => self.parse_literal("ull"),
            _ => perr!(self, InvalidJsonValue),
        }?;
        let slice = self.read.slice_unchecked(start, self.read.index());
        Ok((slice, status))
    }

    #[inline(always)]
    pub(crate) fn parse_trailing(&mut self) -> Result<()> {
        // check exceed
        let exceed = self.read.index() > self.read.as_u8_slice().len();
        if exceed {
            return perr!(self, EofWhileParsing);
        }

        // has_main should marked before skip_space
        let remain = self.read.remain() > 0;
        if !remain {
            return Ok(());
        }

        // note: we use padding chars `x"x` when parsing json into dom.
        // so, we should check the trailing chars is not the padding chars.
        let last = self.skip_space();
        let exceed = self.read.index() > self.read.as_u8_slice().len();
        if last.is_some() && !exceed {
            perr!(self, TrailingCharacters)
        } else {
            Ok(())
        }
    }

    // get_from_object will make reader at the position after target key in JSON object.
    // Advance reader past the value of `target_key` in a JSON object.
    // When `checked` is false, uses fast-path token scanning to skip values.
    fn get_from_object(
        &mut self,
        target_key: &str,
        temp_buf: &mut Vec<u8>,
        checked: bool,
    ) -> Result<()> {
        match self.skip_space() {
            Some(b'{') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON object")),
            None => return perr!(self, EofWhileParsing),
        }

        // deal with the empty object
        match self.get_next_token([b'"', b'}'], 1) {
            Some(b'"') => {}
            Some(b'}') => return perr!(self, GetInEmptyObject),
            None => return perr!(self, EofWhileParsing),
            Some(_) => unreachable!(),
        }

        loop {
            let key = self.parse_string_raw(temp_buf)?;
            self.parse_object_clo()?;
            if key.len() == target_key.len() && key.as_ref() == target_key.as_bytes() {
                return Ok(());
            }

            if checked {
                self.skip_one(true)?;
                match self.skip_space() {
                    Some(b'}') => return perr!(self, GetUnknownKeyInObject),
                    Some(b',') => match self.skip_space() {
                        Some(b'"') => continue,
                        _ => return perr!(self, ExpectObjectKeyOrEnd),
                    },
                    None => return perr!(self, EofWhileParsing),
                    _ => return perr!(self, ExpectedObjectCommaOrEnd),
                };
            } else {
                // skip object,array,string at first (unchecked fast path)
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => unsafe {
                        let _ = self.skip_string_unchecked()?;
                    },
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
                // optimize: direct find the next quote of key or object ending
                match self.get_next_token([b'"', b'}'], 1) {
                    Some(b'"') => continue,
                    Some(b'}') => return perr!(self, GetUnknownKeyInObject),
                    None => return perr!(self, EofWhileParsing),
                    Some(_) => unreachable!(),
                }
            }
        }
    }

    // Advance reader past `index` elements in a JSON array.
    // When `checked` is false, uses fast-path token scanning to skip values.
    fn get_from_array(&mut self, index: usize, checked: bool) -> Result<()> {
        let mut count = index;
        match self.skip_space() {
            Some(b'[') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON array")),
            None => return perr!(self, EofWhileParsing),
        }

        if checked {
            match self.skip_space_peek() {
                Some(b']') => return perr!(self, GetInEmptyArray),
                Some(_) => {}
                None => return perr!(self, EofWhileParsing),
            }
        }

        while count > 0 {
            if checked {
                self.skip_one(true)?;
                match self.skip_space() {
                    Some(b']') => return perr!(self, GetIndexOutOfArray),
                    Some(b',') => {}
                    Some(_) => return perr!(self, ExpectedArrayCommaOrEnd),
                    None => return perr!(self, EofWhileParsing),
                }
                count -= 1;
                match self.skip_space_peek() {
                    Some(_) if count == 0 => return Ok(()),
                    None => return perr!(self, EofWhileParsing),
                    _ => continue,
                }
            } else {
                // skip object,array,string at first (unchecked fast path)
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => unsafe {
                        let _ = self.skip_string_unchecked()?;
                    },
                    Some(b']') => return perr!(self, GetInEmptyArray),
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
                // optimize: direct find the next token
                match self.get_next_token([b']', b','], 1) {
                    Some(b']') => return perr!(self, GetIndexOutOfArray),
                    Some(b',') => {
                        count -= 1;
                        continue;
                    }
                    None => return perr!(self, EofWhileParsing),
                    Some(_) => unreachable!(),
                }
            }
        }

        Ok(())
    }

    pub(crate) fn get_from_with_iter<P: IntoIterator>(
        &mut self,
        path: P,
        checked: bool,
    ) -> Result<(&'de [u8], ParseStatus)>
    where
        P::Item: Index,
    {
        // temp buf reused when parsing each escaped key
        let mut temp_buf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        for jp in path.into_iter() {
            if let Some(key) = jp.as_key() {
                self.get_from_object(key, &mut temp_buf, checked)
            } else if let Some(index) = jp.as_index() {
                self.get_from_array(index, checked)
            } else {
                unreachable!();
            }?;
        }
        self.skip_one(true)
    }

    fn get_many_rec(
        &mut self,
        node: &PointerTreeNode,
        out: &mut Vec<Option<LazyValue<'de>>>,
        strbuf: &mut Vec<u8>,
        remain: &mut usize,
        is_safe: bool,
    ) -> Result<()> {
        // all path has parsed
        if *remain == 0 {
            return Ok(());
        }

        // skip the leading space
        let ch = self.skip_space_peek();
        if ch.is_none() {
            return perr!(self, EofWhileParsing);
        }

        // need write to out, record the start position
        let start = self.read.index();
        let slice: &'de [u8];

        let mut status = ParseStatus::None;
        match &node.children {
            PointerTreeInner::Empty => {
                status = self.skip_one(true)?.1;
            }
            PointerTreeInner::Index(midxs) => {
                self.get_many_index(midxs, strbuf, out, remain, is_safe)?
            }
            PointerTreeInner::Key(mkeys) => {
                self.get_many_keys(mkeys, strbuf, out, remain, is_safe)?
            }
        };

        if !node.order.is_empty() {
            slice = self.read.slice_unchecked(start, self.read.index());
            let lv = LazyValue::new(slice.into(), status.into());
            for p in &node.order {
                out[*p] = Some(lv.clone());
            }
            *remain -= node.order.len();
        }
        Ok(())
    }

    #[allow(clippy::mutable_key_type)]
    fn get_many_keys(
        &mut self,
        mkeys: &MultiKey,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<Option<LazyValue<'de>>>,
        remain: &mut usize,
        checked: bool,
    ) -> Result<()> {
        debug_assert!(strbuf.is_empty());
        match self.skip_space() {
            Some(b'{') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON object")),
            None => return perr!(self, EofWhileParsing),
        }

        // deal with the empty object
        if checked {
            match self.skip_space() {
                Some(b'"') => {}
                Some(b'}') => return perr!(self, GetInEmptyObject),
                _ => return perr!(self, ExpectObjectKeyOrEnd),
            }
        } else {
            match self.get_next_token([b'"', b'}'], 1) {
                Some(b'"') => {}
                Some(b'}') => return perr!(self, GetInEmptyObject),
                None => return perr!(self, EofWhileParsing),
                Some(_) => unreachable!(),
            }
        }

        loop {
            let key = self.parse_str(strbuf)?;
            self.parse_object_clo()?;
            if let Some(val) = mkeys.get(key.deref()) {
                self.get_many_rec(val, out, strbuf, remain, checked)?;
                if *remain == 0 {
                    break;
                }
            } else if checked {
                self.skip_one(true)?;
            } else {
                // skip object,array,string at first (unchecked fast path)
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => unsafe {
                        let _ = self.skip_string_unchecked()?;
                    },
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
            }

            if checked {
                match self.skip_space() {
                    Some(b',') if self.skip_space() == Some(b'"') => continue,
                    Some(b',') => return perr!(self, ExpectObjectKeyOrEnd),
                    Some(b'}') => break,
                    Some(_) => return perr!(self, ExpectedObjectCommaOrEnd),
                    None => return perr!(self, EofWhileParsing),
                }
            } else {
                // optimize: direct find the next quote of key. or object ending
                match self.get_next_token([b'"', b'}'], 1) {
                    Some(b'"') => {}
                    Some(b'}') => break,
                    None => return perr!(self, EofWhileParsing),
                    Some(_) => unreachable!(),
                }
            }
        }

        Ok(())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn remain_str(&self) -> &'de str {
        as_str(self.remain_u8_slice())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn remain_u8_slice(&self) -> &'de [u8] {
        let reader = &self.read;
        let start = reader.index();
        reader.slice_unchecked(start, start + reader.remain())
    }

    fn get_many_index(
        &mut self,
        midx: &MultiIndex,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<Option<LazyValue<'de>>>,
        remain: &mut usize,
        checked: bool,
    ) -> Result<()> {
        match self.skip_space() {
            Some(b'[') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON array")),
            None => return perr!(self, EofWhileParsing),
        }
        let mut index = 0;
        let mut visited = 0;

        match self.skip_space_peek() {
            Some(b']') => return perr!(self, GetInEmptyArray),
            Some(_) => {}
            None => return perr!(self, EofWhileParsing),
        }

        loop {
            if let Some(val) = midx.get(&index) {
                self.get_many_rec(val, out, strbuf, remain, checked)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else if checked {
                self.skip_one(true)?;
            } else {
                // skip object,array,string at first (unchecked fast path)
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => unsafe {
                        let _ = self.skip_string_unchecked()?;
                    },
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
            }

            if checked {
                match self.skip_space() {
                    Some(b']') => break,
                    Some(b',') => {
                        index += 1;
                        continue;
                    }
                    Some(_) => return perr!(self, ExpectedArrayCommaOrEnd),
                    None => return perr!(self, EofWhileParsing),
                }
            } else {
                // optimize: direct find the next token
                match self.get_next_token([b']', b','], 1) {
                    Some(b']') => break,
                    Some(b',') => {
                        index += 1;
                        continue;
                    }
                    None => return perr!(self, EofWhileParsing),
                    Some(_) => unreachable!(),
                }
            }
        }

        // check whether remaining unknown keys
        if visited < midx.len() {
            perr!(self, GetIndexOutOfArray)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_many(
        &mut self,
        tree: &PointerTree,
        is_safe: bool,
    ) -> Result<Vec<Option<LazyValue<'de>>>> {
        let mut strbuf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        let mut remain = tree.size();
        let mut out: Vec<Option<LazyValue<'de>>> = Vec::with_capacity(tree.size());
        out.resize(tree.size(), Option::default());
        let cur = &tree.root;
        self.get_many_rec(cur, &mut out, &mut strbuf, &mut remain, is_safe)?;
        Ok(out)
    }

    #[cold]
    pub fn peek_invalid_type(&mut self, peek: u8, exp: &dyn Expected) -> Error {
        let err = match peek {
            b'n' => {
                if let Err(err) = self.parse_literal("ull") {
                    return err;
                }
                de::Error::invalid_type(Unexpected::Unit, exp)
            }
            b't' => {
                if let Err(err) = self.parse_literal("rue") {
                    return err;
                }
                de::Error::invalid_type(Unexpected::Bool(true), exp)
            }
            b'f' => {
                if let Err(err) = self.parse_literal("alse") {
                    return err;
                }
                de::Error::invalid_type(Unexpected::Bool(false), exp)
            }
            c @ b'-' | c @ b'0'..=b'9' => match self.parse_number(c) {
                Ok(n) => invalid_type_number(&n, exp),
                Err(err) => return err,
            },
            b'"' => {
                let mut scratch = Vec::new();
                match self.parse_str(&mut scratch) {
                    Ok(s) if std::str::from_utf8(s.as_bytes()).is_ok() => {
                        de::Error::invalid_type(Unexpected::Str(&s), exp)
                    }
                    Ok(s) => de::Error::invalid_type(Unexpected::Bytes(s.as_bytes()), exp),
                    Err(err) => return err,
                }
            }
            // for correctness, we will parse the whole object or array.
            b'[' => {
                self.read.backward(1);

                match self.skip_one(true) {
                    Ok(_) => de::Error::invalid_type(Unexpected::Seq, exp),
                    Err(err) => return err,
                }
            }
            b'{' => {
                self.read.backward(1);
                match self.skip_one(true) {
                    Ok(_) => de::Error::invalid_type(Unexpected::Map, exp),
                    Err(err) => return err,
                }
            }
            _ => self.error(ErrorCode::InvalidJsonValue),
        };
        self.fix_position(err)
    }
}

impl<'de, R> Parser<R>
where
    R: Reader<'de>,
{
    pub fn get_by_schema(&mut self, schema: &mut crate::Value) -> Result<()> {
        if !schema.is_object() {
            return perr!(
                self,
                Message(std::borrow::Cow::Borrowed("The schema must be an object"))
            );
        }

        let mut strbuf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        self.get_by_schema_rec(schema, &mut strbuf)
    }

    fn get_by_schema_rec(&mut self, schema: &mut crate::Value, strbuf: &mut Vec<u8>) -> Result<()> {
        let ch = self.skip_space_peek();
        if ch.is_none() {
            return perr!(self, EofWhileParsing);
        }

        let mut should_replace = true;
        let start = self.read.index();

        match (schema.as_object_mut(), ch) {
            (Some(object), Some(b'{')) => {
                let mut key_values = HashMap::new();
                for (key, value) in object.iter_mut() {
                    key_values.insert(key, value);
                }

                // We should replace the schema object if the object is empty
                should_replace = key_values.is_empty();
                if should_replace {
                    self.skip_one(true)?;
                } else {
                    self.read.eat(1);
                    match self.skip_space() {
                        Some(b'"') => {}
                        Some(b'}') => return Ok(()),
                        _ => {
                            return perr!(self, ExpectObjectKeyOrEnd);
                        }
                    }

                    loop {
                        let key = self.parse_str(strbuf)?;
                        self.parse_object_clo()?;
                        if let Some(val) = key_values.get_mut(key.deref()) {
                            self.get_by_schema_rec(val, strbuf)?;
                        } else {
                            self.skip_one(true)?;
                        }

                        match self.skip_space() {
                            Some(b',') => match self.skip_space() {
                                Some(b'"') => continue,
                                _ => return perr!(self, ExpectObjectKeyOrEnd),
                            },
                            Some(b'}') => break,
                            Some(_) => return perr!(self, ExpectedObjectCommaOrEnd),
                            None => return perr!(self, EofWhileParsing),
                        }
                    }
                }
            }
            _ => {
                self.skip_one(true)?;
            }
        }

        let end = self.read.index();
        if should_replace && start < end {
            let slice = self.read.slice_unchecked(start, end);
            *schema = crate::from_slice(slice)?;
        }
        Ok(())
    }
}
