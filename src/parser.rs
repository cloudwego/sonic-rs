use std::{
    num::NonZeroU8,
    ops::Deref,
    slice::{from_raw_parts, from_raw_parts_mut},
    str::from_utf8_unchecked,
};

use arrayref::array_ref;
use faststr::FastStr;
use serde::de::{self, Expected, Unexpected};
use smallvec::SmallVec;

use super::reader::{Reader, Reference};
#[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
use crate::util::simd::bits::NeonBits;
use crate::{
    error::{
        Error,
        ErrorCode::{self, *},
        Result,
    },
    index::Index,
    pointer::{
        tree::{MultiIndex, MultiKey, PointerTreeInner, PointerTreeNode},
        JsonPointer, PointerTree,
    },
    util::{
        arc::Arc,
        arch::{get_nonspace_bits, prefix_xor},
        num::{parse_number, ParserNumber},
        simd::{i8x32, m8x32, u8x32, u8x64, Mask, Simd},
        string::*,
        unicode::{codepoint_to_utf8, hex_to_u32_nocheck},
    },
    value::{shared::Shared, visitor::JsonVisitor},
    JsonType, LazyValue,
};

pub(crate) const DEFAULT_KEY_BUF_CAPACITY: usize = 128;
pub(crate) fn as_str(data: &[u8]) -> &str {
    unsafe { from_utf8_unchecked(data) }
}

#[inline(always)]
fn get_escaped_branchless_u32(prev_escaped: &mut u32, backslash: u32) -> u32 {
    const EVEN_BITS: u32 = 0x5555_5555;
    let backslash = backslash & (!*prev_escaped);
    let follows_escape = backslash << 1 | *prev_escaped;
    let odd_sequence_starts = backslash & !EVEN_BITS & !follows_escape;
    let (sequences_starting_on_even_bits, overflow) =
        odd_sequence_starts.overflowing_add(backslash);
    *prev_escaped = overflow as u32;
    let invert_mask = sequences_starting_on_even_bits << 1;
    (EVEN_BITS ^ invert_mask) & follows_escape
}

// convert $int to u32 for JsonPointer.
macro_rules! perr {
    ($self:ident, $err:expr) => {{
        Err($self.error($err))
    }};
}

macro_rules! check_visit {
    ($self:ident, $e:expr $(,)?) => {
        if !($e) {
            return perr!($self, UnexpectedVisitType);
        }
    };
}

#[inline(always)]
fn get_escaped_branchless_u64(prev_escaped: &mut u64, backslash: u64) -> u64 {
    const EVEN_BITS: u64 = 0x5555_5555_5555_5555;
    let backslash = backslash & (!*prev_escaped);
    let follows_escape = backslash << 1 | *prev_escaped;
    let odd_sequence_starts = backslash & !EVEN_BITS & !follows_escape;
    let (sequences_starting_on_even_bits, overflow) =
        odd_sequence_starts.overflowing_add(backslash);
    *prev_escaped = overflow as u64;
    let invert_mask = sequences_starting_on_even_bits << 1;
    (EVEN_BITS ^ invert_mask) & follows_escape
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

pub(crate) struct Parser<R> {
    pub(crate) read: R,
    error_index: usize,                     // mark the error position
    nospace_bits: u64,                      // SIMD marked nospace bitmap
    nospace_start: isize,                   // the start position of nospace_bits
    pub(crate) shared: Option<Arc<Shared>>, // the shared allocator for `Value`
}

/// Records the parse status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseStatus {
    None,
    HasEscaped,
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
            shared: None,
        }
    }

    #[inline(always)]
    fn error_index(&self) -> usize {
        // when parsing strings , we need record the error position.
        // it must be smaller than reader.index().
        std::cmp::min(self.error_index, self.read.index().saturating_sub(1))
    }

    /// Error caused by a byte from next_char().
    #[cold]
    pub(crate) fn error(&self, mut reason: ErrorCode) -> Error {
        // check invalid utf8 here at first
        // FIXME: maybe has invalid utf8 when deserialzing into byte, and just bytes has other
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
        Error::syntax(reason, self.read.as_u8_slice(), index)
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
    pub(crate) fn parse_number(&mut self, first: u8) -> Result<ParserNumber> {
        let reader = &mut self.read;
        let neg = first == b'-';
        let mut now = reader.index() - (!neg as usize);
        let data = reader.as_u8_slice();
        let ret = parse_number(data, &mut now, neg);
        reader.set_index(now);
        match ret {
            Err(code) => perr!(self, code),
            Ok(num) => Ok(num),
        }
    }

    #[inline(always)]
    fn parse_string<V>(&mut self, visitor: &mut V, strbuf: &mut Vec<u8>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let ok = match self.parse_str_impl(strbuf)? {
            Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
            Reference::Copied(s) => visitor.visit_str(s),
        };
        if !ok {
            perr!(self, UnexpectedVisitType)
        } else {
            Ok(())
        }
    }

    // TODO: optimize me, avoid clone twice.
    #[inline(always)]
    fn parse_string_owned<V>(&mut self, visitor: &mut V, strbuf: &mut Vec<u8>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let rs = self.parse_str_impl(strbuf)?;
        if !visitor.visit_str(rs.as_ref()) {
            perr!(self, UnexpectedVisitType)
        } else {
            Ok(())
        }
    }

    #[inline(always)]
    fn parse_string_inplace<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        unsafe {
            let mut src = self.read.cur_ptr();
            let start = self.read.cur_ptr();
            let cnt = parse_string_inplace(&mut src).map_err(|e| self.error(e))?;
            self.read.set_ptr(src);
            let slice = from_raw_parts(start, cnt);
            let s = from_utf8_unchecked(slice);
            check_visit!(self, visitor.visit_borrowed_str(s));
        }
        Ok(())
    }

    #[inline(always)]
    fn parse_number_visit<V>(&mut self, first: u8, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        #[cfg(feature = "arbitrary_precision")]
        {
            let start = self.read.index() - 1;
            self.skip_number(first)?;
            let slice = self.read.slice_unchecked(start, self.read.index());
            check_visit!(self, visitor.visit_raw_number(as_str(slice)));
            Ok(())
        }

        #[cfg(not(feature = "arbitrary_precision"))]
        {
            let ok = match self.parse_number(first)? {
                ParserNumber::Float(f) => visitor.visit_f64(f),
                ParserNumber::Unsigned(f) => visitor.visit_u64(f),
                ParserNumber::Signed(f) => visitor.visit_i64(f),
            };
            check_visit!(self, ok);
            Ok(())
        }
    }

    #[inline(always)]
    fn parse_number_inplace<V>(&mut self, first: u8, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        #[cfg(feature = "arbitrary_precision")]
        {
            let start = self.read.index() - 1;
            self.skip_number(first)?;
            let slice = self.read.slice_unchecked(start, self.read.index());
            check_visit!(self, visitor.visit_borrowed_raw_number(as_str(slice)));
            Ok(())
        }

        #[cfg(not(feature = "arbitrary_precision"))]
        {
            let ok = match self.parse_number(first)? {
                ParserNumber::Float(f) => visitor.visit_f64(f),
                ParserNumber::Unsigned(f) => visitor.visit_u64(f),
                ParserNumber::Signed(f) => visitor.visit_i64(f),
            };
            check_visit!(self, ok);
            Ok(())
        }
    }

    #[inline(always)]
    fn parse_array<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        // parsing empty array
        check_visit!(self, visitor.visit_array_start(0));

        let mut first = match self.skip_space() {
            Some(b']') => {
                check_visit!(self, visitor.visit_array_end(0));
                return Ok(());
            }
            first => first,
        };

        let mut count = 0;
        loop {
            match first {
                Some(c @ b'-' | c @ b'0'..=b'9') => self.parse_number_inplace(c, visitor),
                Some(b'"') => self.parse_string_inplace(visitor),
                Some(b'{') => self.parse_object(visitor),
                Some(b'[') => self.parse_array(visitor),
                Some(first) => self.parse_literal_visit(first, visitor),
                None => perr!(self, EofWhileParsing),
            }?;
            count += 1;
            first = match self.skip_space() {
                Some(b']') => {
                    check_visit!(self, visitor.visit_array_end(count));
                    return Ok(());
                }
                Some(b',') => self.skip_space(),
                _ => return perr!(self, ExpectedArrayCommaOrEnd),
            };
        }
    }

    #[inline(always)]
    fn parse_key<V>(&mut self, visitor: &mut V, strbuf: &mut Vec<u8>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let ok = match self.parse_str_impl(strbuf)? {
            Reference::Borrowed(s) => visitor.visit_borrowed_key(s),
            Reference::Copied(s) => visitor.visit_key(s),
        };
        check_visit!(self, ok);
        Ok(())
    }

    #[inline(always)]
    fn parse_object<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        // parsing empty object
        let mut count: usize = 0;
        check_visit!(self, visitor.visit_object_start(0));
        match self.skip_space() {
            Some(b'}') => {
                check_visit!(self, visitor.visit_object_end(0));
                return Ok(());
            }
            Some(b'"') => {}
            _ => {
                return perr!(self, ExpectObjectKeyOrEnd);
            }
        }

        // loop for each object key and value
        loop {
            self.parse_string_inplace(visitor)?;
            self.parse_object_clo()?;
            self.parse_value(visitor)?;
            count += 1;
            match self.skip_space() {
                Some(b'}') => {
                    check_visit!(self, visitor.visit_object_end(count));
                    return Ok(());
                }
                Some(b',') => match self.skip_space() {
                    Some(b'"') => continue,
                    _ => {
                        return perr!(self, ExpectObjectKeyOrEnd);
                    }
                },
                _ => return perr!(self, ExpectedArrayCommaOrEnd),
            }
        }
    }

    #[inline(always)]
    fn parse_literal_visit<V>(&mut self, first: u8, visitor: &mut V) -> Result<()>
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
                perr!(self, InvalidLiteral)
            } else {
                let ok = match first {
                    b't' => visitor.visit_bool(true),
                    b'f' => visitor.visit_bool(false),
                    b'n' => visitor.visit_null(),
                    _ => unreachable!(),
                };
                check_visit!(self, ok);
                Ok(())
            }
        } else {
            perr!(self, EofWhileParsing)
        }
    }

    #[inline]
    pub(crate) fn get_json_type(&mut self) -> Result<JsonType> {
        match self.skip_space_peek() {
            Some(b'-') | Some(b'0'..=b'9') => Ok(JsonType::Number),
            Some(b'"') => Ok(JsonType::String),
            Some(b'{') => Ok(JsonType::Object),
            Some(b'[') => Ok(JsonType::Array),
            Some(b't') | Some(b'f') => Ok(JsonType::Boolean),
            Some(b'n') => Ok(JsonType::Null),
            _ => perr!(self, EofWhileParsing),
        }
    }

    #[inline]
    pub(crate) fn parse_array_elem_lazy(
        &mut self,
        first: &mut bool,
        check: bool,
    ) -> Result<Option<(&'de [u8], bool)>> {
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
        let (raw, status) = if check {
            self.skip_one()
        } else {
            self.skip_one_unchecked()
        }?;
        Ok(Some((raw, status == ParseStatus::HasEscaped)))
    }

    #[inline]
    pub(crate) fn parse_entry_lazy(
        &mut self,
        strbuf: &mut Vec<u8>,
        first: &mut bool,
        check: bool,
    ) -> Result<Option<(FastStr, &'de [u8], bool)>> {
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

        let key = match self.parse_str_impl(strbuf)? {
            Reference::Borrowed(s) => FastStr::new(s),
            Reference::Copied(s) => FastStr::new(s),
        };
        self.parse_object_clo()?;
        let (raw, status) = if check {
            self.skip_one()
        } else {
            self.skip_one_unchecked()
        }?;
        Ok(Some((key, raw, status == ParseStatus::HasEscaped)))
    }

    // Not use non-recurse version here, because it maybe 5% slower than recurse version.
    #[inline(always)]
    pub(crate) fn parse_value<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        match self.skip_space() {
            Some(c @ b'-' | c @ b'0'..=b'9') => self.parse_number_inplace(c, visitor),
            Some(b'"') => self.parse_string_inplace(visitor),
            Some(b'{') => self.parse_object(visitor),
            Some(b'[') => self.parse_array(visitor),
            Some(first) => self.parse_literal_visit(first, visitor),
            None => perr!(self, EofWhileParsing),
        }
    }

    pub(crate) fn parse_value_without_padding<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        const COMMON_DEPTH: usize = 20;
        const ARR_MASK: u32 = 1u32 << 31;
        const OBJ_MASK: u32 = 0u32;
        let mut depth = SmallVec::<[u32; COMMON_DEPTH]>::new();
        let mut c: u8;

        enum Fsm {
            ScopeEnd,
            ArrVal,
            ObjKey,
        }

        let mut strbuf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        let mut state;
        match self.skip_space2() {
            b'[' => {
                check_visit!(self, visitor.visit_array_start(0));
                depth.push(ARR_MASK);
                c = self.skip_space2();
                if c == b']' {
                    check_visit!(self, visitor.visit_array_end(0));
                    state = Fsm::ScopeEnd;
                } else {
                    state = Fsm::ArrVal;
                }
            }
            b'{' => {
                check_visit!(self, visitor.visit_object_start(0));
                depth.push(OBJ_MASK);
                c = self.skip_space2();
                if c == b'}' {
                    check_visit!(self, visitor.visit_object_end(0));
                    state = Fsm::ScopeEnd;
                } else {
                    state = Fsm::ObjKey;
                }
            }
            c @ b'-' | c @ b'0'..=b'9' => return self.parse_number_visit(c, visitor),
            b'"' => return self.parse_string_owned(visitor, &mut strbuf),
            0 => return perr!(self, EofWhileParsing),
            first => return self.parse_literal_visit(first, visitor),
        }

        loop {
            match state {
                Fsm::ArrVal => {
                    'arr_val: loop {
                        match c {
                            b'{' => {
                                check_visit!(self, visitor.visit_object_start(0));
                                depth.push(OBJ_MASK);
                                c = self.skip_space2();
                                if c == b'}' {
                                    check_visit!(self, visitor.visit_object_end(0));
                                    state = Fsm::ScopeEnd;
                                } else {
                                    state = Fsm::ObjKey;
                                }
                                break 'arr_val;
                            }
                            b'[' => {
                                check_visit!(self, visitor.visit_array_start(0));
                                depth.push(ARR_MASK);
                                c = self.skip_space2();
                                if c == b']' {
                                    check_visit!(self, visitor.visit_array_end(0));
                                    state = Fsm::ScopeEnd;
                                    break 'arr_val;
                                }

                                continue 'arr_val;
                            }
                            c @ b'-' | c @ b'0'..=b'9' => self.parse_number_visit(c, visitor)?,
                            b'"' => self.parse_string_owned(visitor, &mut strbuf)?,
                            first => self.parse_literal_visit(first, visitor)?,
                        }
                        // count after array primitive value end
                        let len = depth.len();
                        depth[len - 1] += 1;
                        match self.skip_space2() {
                            b',' => {
                                c = self.skip_space2();
                                continue 'arr_val;
                            }
                            b']' => {
                                let back = depth[depth.len() - 1];
                                check_visit!(
                                    self,
                                    visitor.visit_array_end((back & (ARR_MASK - 1)) as usize)
                                );
                                state = Fsm::ScopeEnd;
                                break 'arr_val;
                            }
                            _ => return perr!(self, ExpectedArrayCommaOrEnd),
                        }
                    }
                }
                Fsm::ObjKey => {
                    'obj_key: loop {
                        if c != b'"' {
                            return perr!(self, ExpectObjectKeyOrEnd);
                        }
                        self.parse_string_owned(visitor, &mut strbuf)?;
                        self.parse_object_clo()?;
                        match self.skip_space2() {
                            b'{' => {
                                check_visit!(self, visitor.visit_object_start(0));
                                depth.push(OBJ_MASK);
                                c = self.skip_space2();
                                if c == b'}' {
                                    check_visit!(self, visitor.visit_object_end(0));
                                    state = Fsm::ScopeEnd;
                                    break 'obj_key;
                                }

                                continue 'obj_key;
                            }
                            b'[' => {
                                check_visit!(self, visitor.visit_array_start(0));
                                depth.push(ARR_MASK);
                                c = self.skip_space2();
                                if c == b']' {
                                    check_visit!(self, visitor.visit_array_end(0));
                                    state = Fsm::ScopeEnd;
                                } else {
                                    state = Fsm::ArrVal;
                                }
                                break 'obj_key;
                            }
                            c @ b'-' | c @ b'0'..=b'9' => self.parse_number_visit(c, visitor)?,
                            b'"' => self.parse_string_owned(visitor, &mut strbuf)?,
                            first => self.parse_literal_visit(first, visitor)?,
                        }
                        // count after object primitive value end
                        let len = depth.len();
                        depth[len - 1] += 1;
                        match self.skip_space2() {
                            b',' => {
                                c = self.skip_space2();

                                continue 'obj_key;
                            }
                            b'}' => {
                                let back = depth[depth.len() - 1];
                                check_visit!(
                                    self,
                                    visitor.visit_object_end((back & (ARR_MASK - 1)) as usize)
                                );
                                state = Fsm::ScopeEnd;
                                break 'obj_key;
                            }
                            _ => return perr!(self, ExpectedArrayCommaOrEnd),
                        }
                    }
                }
                Fsm::ScopeEnd => {
                    'scope_end: loop {
                        depth.pop();
                        if depth.is_empty() {
                            // Note: we will not check trailing characters
                            // because get_from maybe returns all remaining bytes.
                            return Ok(());
                        }
                        // count after container value end
                        let len = depth.len();
                        depth[len - 1] += 1;
                        c = self.skip_space2();
                        if (depth[len - 1] & ARR_MASK) != 0 {
                            // parent is array
                            match c {
                                b',' => {
                                    c = self.skip_space2();
                                    state = Fsm::ArrVal;

                                    break 'scope_end;
                                }
                                b']' => {
                                    let back = depth[depth.len() - 1];
                                    check_visit!(
                                        self,
                                        visitor.visit_array_end((back & (ARR_MASK - 1)) as usize)
                                    );

                                    continue 'scope_end;
                                }
                                _ => return perr!(self, ExpectedArrayCommaOrEnd),
                            }
                        } else {
                            // parent is object
                            match c {
                                b',' => {
                                    c = self.skip_space2();
                                    state = Fsm::ObjKey;

                                    break 'scope_end;
                                }
                                b'}' => {
                                    let back = depth[depth.len() - 1];
                                    check_visit!(
                                        self,
                                        visitor.visit_object_end((back & (ARR_MASK - 1)) as usize)
                                    );

                                    continue 'scope_end;
                                }
                                _ => return perr!(self, ExpectedObjectCommaOrEnd),
                            }
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    pub(crate) fn parse_str_impl<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<Reference<'de, 'own, str>> {
        let slice = self.parse_string_raw(buf)?;
        Ok(match slice {
            Reference::Copied(buf) => Reference::Copied(unsafe { from_utf8_unchecked(buf) }),
            Reference::Borrowed(buf) => Reference::Borrowed(unsafe { from_utf8_unchecked(buf) }),
        })
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
                    return perr!(self, InvalidSurrogateUnicodeCodePoint);
                }
                unsafe { hex_to_u32_nocheck(&*(asc.as_ptr().add(2) as *const _ as *const [u8; 4])) }
            } else {
                return perr!(self, EofWhileParsing);
            };

            /* calcute the real code point */
            let low_bit = point2.wrapping_sub(0xdc00);
            if (low_bit >> 10) != 0 {
                // invalid surrogate
                return perr!(self, InvalidSurrogateUnicodeCodePoint);
            }

            Ok((((point1 - 0xd800) << 10) | low_bit).wrapping_add(0x10000))
        } else if (0xDC00..0xE000).contains(&point1) {
            // invalid surrogate
            perr!(self, InvalidSurrogateUnicodeCodePoint)
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
    ) -> Result<Reference<'de, 'own, [u8]>> {
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
                return Ok(Reference::Copied(buf.as_slice()));
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
                    return Ok(Reference::Copied(buf.as_slice()));
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
    ) -> Result<Reference<'de, 'own, [u8]>> {
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
                return Ok(Reference::Borrowed(
                    self.read.slice_unchecked(start, self.read.index() - 1),
                ));
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
                    return Ok(Reference::Borrowed(
                        self.read.slice_unchecked(start, self.read.index() - 1),
                    ));
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
        const LANS: usize = u8x32::lanes();
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
    fn skip_string_impl(&mut self) -> Result<ParseStatus> {
        const LANS: usize = u8x32::lanes();
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

    #[inline(always)]
    fn skip_string_unchecked(&mut self) -> Result<ParseStatus> {
        self.skip_string_impl()
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
        const LANS: usize = u8x32::lanes();

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
            self.skip_one()?;

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
            self.skip_one()?;
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
            let input = array_ref![chunk, 0, 64];
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
        unsafe {
            let n = reader.remain();
            remain[..n].copy_from_slice(reader.peek_n(n).unwrap_unchecked());
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
    pub(crate) fn skip_space(&mut self) -> Option<u8> {
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
            let chunk = array_ref![chunk, 0, 64];
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
    pub(crate) fn skip_space2(&mut self) -> u8 {
        let reader = &mut self.read;
        // fast path 1: for nospace or single space
        // most JSON is like ` "name": "balabala" `
        if let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return ch;
            }
        }
        if let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return ch;
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

                return ch;
            } else {
                // we can still fast skip the marked space in here.
                reader.set_index(self.nospace_start as usize + 64);
            }
        }

        // then we use simd to accelerate skipping space
        while let Some(chunk) = reader.peek_n(64) {
            let chunk = array_ref![chunk, 0, 64];
            let bitmap = unsafe { get_nonspace_bits(chunk) };
            if bitmap != 0 {
                self.nospace_bits = bitmap;
                self.nospace_start = reader.index() as isize;
                let cnt = bitmap.trailing_zeros() as usize;
                let ch = chunk[cnt];
                reader.eat(cnt + 1);

                return ch;
            }
            reader.eat(64)
        }

        while let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return ch;
            }
        }
        0
    }

    #[inline(always)]
    pub(crate) fn skip_space_peek(&mut self) -> Option<u8> {
        let ret = self.skip_space()?;
        self.read.backward(1);
        Some(ret)
    }

    #[inline(always)]
    pub(crate) fn parse_literal(&mut self, literal: &str) -> Result<()> {
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
    pub(crate) fn skip_number(&mut self, mut first: u8) -> Result<()> {
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
            Some(b'0'..=b'9') => {
                self.read.eat(1);
            }
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
        while let Some(chunk) = self.read.peek_n(32) {
            let v = unsafe { i8x32::from_slice_unaligned_unchecked(chunk) };
            let zero = i8x32::splat(b'0' as i8);
            let nine = i8x32::splat(b'9' as i8);
            let nondigits = (zero.gt(&v) | v.gt(&nine)).bitmask();
            if nondigits != 0 {
                let cnt = nondigits.trailing_zeros() as usize;
                let ch = chunk[cnt];
                if ch == b'.' && !is_float {
                    self.read.eat(cnt + 1);
                    // check the first digit after the dot
                    self.skip_single_digit()?;

                    let traversed = cnt + 2;
                    // check the remaining digits
                    let nondigts = nondigits.wrapping_shr((traversed) as u32);
                    if nondigts != 0 {
                        while let Some(ch) = self.read.peek() {
                            if ch == b'e' || ch == b'E' {
                                self.read.eat(1);
                                return self.skip_exponent();
                            } else if ch.is_ascii_digit() {
                                self.read.eat(1);
                                continue;
                            } else {
                                return Ok(());
                            }
                        }
                    } else {
                        // long digits
                        self.read.eat(32 - traversed);
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
            } else {
                // long digits
                self.read.eat(32);
            }
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

    #[inline(always)]
    pub(crate) fn skip_one(&mut self) -> Result<(&'de [u8], ParseStatus)> {
        let ch = self.skip_space();
        let start = self.read.index() - 1;
        let mut status = ParseStatus::None;
        match ch {
            Some(c @ b'-' | c @ b'0'..=b'9') => self.skip_number(c),
            Some(b'"') => {
                status = self.skip_string()?;
                Ok(())
            }
            Some(b'{') => self.skip_object(),
            Some(b'[') => self.skip_array(),
            Some(b't') => self.parse_literal("rue"),
            Some(b'f') => self.parse_literal("alse"),
            Some(b'n') => self.parse_literal("ull"),
            Some(_) => perr!(self, InvalidJsonValue),
            None => perr!(self, EofWhileParsing),
        }?;
        let slice = self.read.slice_unchecked(start, self.read.index());
        Ok((slice, status))
    }

    #[inline(always)]
    pub(crate) fn skip_one_unchecked(&mut self) -> Result<(&'de [u8], ParseStatus)> {
        let ch = self.skip_space();
        let start = self.read.index() - 1;
        let mut status = ParseStatus::None;
        match ch {
            Some(b'-' | b'0'..=b'9') => self.skip_number_unsafe(),
            Some(b'"') => {
                status = self.skip_string_unchecked()?;
                Ok(())
            }
            Some(b'{') => self.skip_container(b'{', b'}'),
            Some(b'[') => self.skip_container(b'[', b']'),
            Some(b't') => self.parse_literal("rue"),
            Some(b'f') => self.parse_literal("alse"),
            Some(b'n') => self.parse_literal("ull"),
            Some(_) => perr!(self, InvalidJsonValue),
            None => perr!(self, EofWhileParsing),
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
        match last {
            Some(_) if remain & !exceed => perr!(self, TrailingCharacters),
            _ => Ok(()),
        }
    }

    // get_from_object will make reader at the position after target key in JSON object.
    #[inline(always)]
    fn get_from_object(&mut self, target_key: &str, temp_buf: &mut Vec<u8>) -> Result<()> {
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

            // skip object,array,string at first
            match self.skip_space() {
                Some(b'{') => self.skip_container(b'{', b'}')?,
                Some(b'[') => self.skip_container(b'[', b']')?,
                Some(b'"') => {
                    let _ = self.skip_string_unchecked()?;
                }
                None => return perr!(self, EofWhileParsing),
                _ => {}
            };

            // optimize: direct find the next quote of key. or object ending
            match self.get_next_token([b'"', b'}'], 1) {
                Some(b'"') => continue,
                Some(b'}') => return perr!(self, GetUnknownKeyInObject),
                None => return perr!(self, EofWhileParsing),
                Some(_) => unreachable!(),
            }
        }
    }

    // get_from_object will make reader at the position after target key in JSON object.
    #[inline(always)]
    fn get_from_object_checked(&mut self, target_key: &str, temp_buf: &mut Vec<u8>) -> Result<()> {
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

            self.skip_one()?;

            match self.skip_space() {
                Some(b'}') => return perr!(self, GetUnknownKeyInObject),
                Some(b',') => match self.skip_space() {
                    Some(b'"') => continue,
                    _ => return perr!(self, ExpectObjectKeyOrEnd),
                },
                None => return perr!(self, EofWhileParsing),
                _ => return perr!(self, ExpectedObjectCommaOrEnd),
            };
        }
    }

    #[inline(always)]
    fn get_from_array_checked(&mut self, index: usize) -> Result<()> {
        let mut count = index;
        match self.skip_space() {
            Some(b'[') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON array")),
            None => return perr!(self, EofWhileParsing),
        }

        match self.skip_space_peek() {
            Some(b']') => return perr!(self, GetInEmptyArray),
            Some(_) => {}
            None => return perr!(self, EofWhileParsing),
        }

        while count > 0 {
            self.skip_one()?;

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
        }

        // index is 0, just skipped '[' and return
        Ok(())
    }

    // get_from_array will make reader at the position after target index in JSON array.
    #[inline(always)]
    fn get_from_array(&mut self, index: usize) -> Result<()> {
        let mut count = index;
        match self.skip_space() {
            Some(b'[') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON array")),
            None => return perr!(self, EofWhileParsing),
        }
        while count > 0 {
            // skip object,array,string at first
            match self.skip_space() {
                Some(b'{') => self.skip_container(b'{', b'}')?,
                Some(b'[') => self.skip_container(b'[', b']')?,
                Some(b'"') => {
                    let _ = self.skip_string_unchecked()?;
                }
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
        // special case: `[]` will report error when skip one later.
        Ok(())
    }

    pub(crate) fn get_from(&mut self, path: &JsonPointer) -> Result<(&'de [u8], ParseStatus)> {
        self.get_from_with_iter(path.iter())
    }

    pub(crate) fn get_from_with_iter<P: IntoIterator>(
        &mut self,
        path: P,
    ) -> Result<(&'de [u8], ParseStatus)>
    where
        P::Item: Index,
    {
        // temp buf reused when parsing each escaped key
        let mut temp_buf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        for jp in path.into_iter() {
            if let Some(key) = jp.as_key() {
                self.get_from_object(key, &mut temp_buf)
            } else if let Some(index) = jp.as_index() {
                self.get_from_array(index)
            } else {
                unreachable!();
            }?;
        }
        self.skip_one()
    }

    pub(crate) fn get_from_with_iter_checked<P: IntoIterator>(
        &mut self,
        path: P,
    ) -> Result<(&'de [u8], ParseStatus)>
    where
        P::Item: Index,
    {
        // temp buf reused when parsing each escaped key
        let mut temp_buf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        for jp in path.into_iter() {
            if let Some(key) = jp.as_key() {
                self.get_from_object_checked(key, &mut temp_buf)
            } else if let Some(index) = jp.as_index() {
                self.get_from_array_checked(index)
            } else {
                unreachable!();
            }?;
        }
        self.skip_one()
    }

    fn get_many_rec(
        &mut self,
        node: &PointerTreeNode,
        out: &mut Vec<LazyValue<'de>>,
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
                status = self.skip_one()?.1;
            }
            PointerTreeInner::Index(midxs) => {
                if is_safe {
                    self.get_many_index(midxs, strbuf, out, remain)?
                } else {
                    self.get_many_index_unchecked(midxs, strbuf, out, remain)?
                }
            }
            PointerTreeInner::Key(mkeys) => {
                if is_safe {
                    self.get_many_keys(mkeys, strbuf, out, remain)?
                } else {
                    self.get_many_keys_unchecked(mkeys, strbuf, out, remain)?
                }
            }
        };

        if !node.order.is_empty() {
            slice = self.read.slice_unchecked(start, self.read.index());
            let lv = LazyValue::new(slice.into(), status == ParseStatus::HasEscaped)?;
            for p in &node.order {
                out[*p] = lv.clone();
            }
            *remain -= node.order.len();
        }
        Ok(())
    }

    #[allow(clippy::mutable_key_type)]
    fn get_many_keys_unchecked(
        &mut self,
        mkeys: &MultiKey,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<LazyValue<'de>>,
        remain: &mut usize,
    ) -> Result<()> {
        debug_assert!(strbuf.is_empty());
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

        let mut visited = 0;
        loop {
            let key = self.parse_str_impl(strbuf)?;
            self.parse_object_clo()?;
            if let Some(val) = mkeys.get(key.deref()) {
                self.get_many_rec(val, out, strbuf, remain, false)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else {
                // skip object,array,string at first
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => {
                        let _ = self.skip_string_unchecked()?;
                    }
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
            }

            // optimize: direct find the next quote of key. or object ending
            match self.get_next_token([b'"', b'}'], 1) {
                Some(b'"') => {}
                Some(b'}') => break,
                None => return perr!(self, EofWhileParsing),
                Some(_) => unreachable!(),
            }
        }

        // check whether remaining unknown keys
        if visited < mkeys.len() {
            perr!(self, GetUnknownKeyInObject)
        } else {
            Ok(())
        }
    }

    #[allow(clippy::mutable_key_type)]
    fn get_many_keys(
        &mut self,
        mkeys: &MultiKey,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<LazyValue<'de>>,
        remain: &mut usize,
    ) -> Result<()> {
        debug_assert!(strbuf.is_empty());
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

        let mut visited = 0;
        loop {
            let key = self.parse_str_impl(strbuf)?;
            self.parse_object_clo()?;
            if let Some(val) = mkeys.get(key.deref()) {
                // parse the child point tree
                self.get_many_rec(val, out, strbuf, remain, true)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else {
                self.skip_one()?;
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

        // check whether remaining unknown keys
        if visited < mkeys.len() {
            perr!(self, GetUnknownKeyInObject)
        } else {
            Ok(())
        }
    }

    pub(crate) fn remain_str(&self) -> &'de str {
        as_str(self.remain_u8_slice())
    }

    pub(crate) fn remain_u8_slice(&self) -> &'de [u8] {
        let reader = &self.read;
        let start = reader.index();
        reader.slice_unchecked(start, start + reader.remain())
    }

    fn get_many_index_unchecked(
        &mut self,
        midx: &MultiIndex,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<LazyValue<'de>>,
        remain: &mut usize,
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
            None => return perr!(self, EofWhileParsing),
            _ => {}
        };

        loop {
            if let Some(val) = midx.get(&index) {
                self.get_many_rec(val, out, strbuf, remain, false)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else {
                // skip object,array,string at first
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => {
                        let _ = self.skip_string_unchecked()?;
                    }
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
            }

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

        // check whether remaining unknown keys
        if visited < midx.len() {
            perr!(self, GetIndexOutOfArray)
        } else {
            Ok(())
        }
    }

    fn get_many_index(
        &mut self,
        midx: &MultiIndex,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<LazyValue<'de>>,
        remain: &mut usize,
    ) -> Result<()> {
        match self.skip_space() {
            Some(b'[') => {}
            Some(peek) => return Err(self.peek_invalid_type(peek, &"a JSON array")),
            None => return perr!(self, EofWhileParsing),
        }
        let mut index = 0;
        let mut visited = 0;

        // check empty array
        match self.skip_space_peek() {
            Some(b']') => return perr!(self, GetInEmptyArray),
            Some(_) => {}
            None => return perr!(self, EofWhileParsing),
        }

        loop {
            if let Some(val) = midx.get(&index) {
                self.get_many_rec(val, out, strbuf, remain, true)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else {
                self.skip_one()?;
            }

            match self.skip_space() {
                Some(b']') => break,
                Some(b',') => {
                    index += 1;
                    continue;
                }
                Some(_) => return perr!(self, ExpectedArrayCommaOrEnd),
                None => return perr!(self, EofWhileParsing),
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
    ) -> Result<Vec<LazyValue<'de>>> {
        let mut strbuf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        let mut remain = tree.size();
        let mut out: Vec<LazyValue<'de>> = Vec::with_capacity(tree.size());
        out.resize(tree.size(), LazyValue::default());
        let cur = &tree.root;
        self.get_many_rec(cur, &mut out, &mut strbuf, &mut remain, is_safe)?;
        Ok(out)
    }

    #[inline]
    pub(crate) fn get_shared_inc_count(&mut self) -> Arc<Shared> {
        if self.shared.is_none() {
            self.shared = Some(Arc::new(Shared::new()));
        }
        unsafe { self.shared.as_ref().unwrap_unchecked().clone() }
    }

    #[cold]
    pub(crate) fn peek_invalid_type(&mut self, peek: u8, exp: &dyn Expected) -> Error {
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
                Ok(n) => n.invalid_type(exp),
                Err(err) => return err,
            },
            b'"' => {
                let mut scratch = Vec::new();
                match self.parse_str_impl(&mut scratch) {
                    Ok(s) => de::Error::invalid_type(Unexpected::Str(&s), exp),
                    Err(err) => return err,
                }
            }
            // for correctness, we will parse the whole object or array.
            b'[' => {
                self.read.backward(1);
                match self.skip_one() {
                    Ok(_) => de::Error::invalid_type(Unexpected::Seq, exp),
                    Err(err) => return err,
                }
            }
            b'{' => {
                self.read.backward(1);
                match self.skip_one() {
                    Ok(_) => de::Error::invalid_type(Unexpected::Map, exp),
                    Err(err) => return err,
                }
            }
            _ => self.error(ErrorCode::InvalidJsonValue),
        };
        self.fix_position(err)
    }
}
