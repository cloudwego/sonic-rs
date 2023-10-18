use super::reader::{Reader, Reference};
use crate::error::ErrorCode::{self, *};
use crate::error::{Error, Result};
use crate::pointer::{
    tree::MultiIndex, tree::MultiKey, tree::PointerTreeInner, tree::PointerTreeNode, PointerTrait,
};
use crate::pointer::{JsonPointer, PointerTree};
use crate::util::arch::{get_nonspace_bits, prefix_xor};
use crate::util::num::{parse_number_unchecked, ParserNumber};
use crate::util::string::parse_valid_escaped_string;
use crate::util::string::*;
use crate::visitor::JsonVisitor;
use crate::JsonType;
use arrayref::array_ref;
use faststr::FastStr;
use packed_simd::{m8x32, u8x32, u8x64};
use smallvec::SmallVec;
use std::num::NonZeroU8;
use std::ops::Deref;
use std::slice::from_raw_parts;
use std::str::from_utf8_unchecked;

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

pub(crate) use perr;

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
fn is_whitespace(ch: u8) -> bool {
    ch == b' ' || ch == b'\r' || ch == b'\n' || ch == b'\t'
}

#[inline(always)]
fn get_string_bits(data: &[u8; 64], prev_instring: &mut u64, prev_escaped: &mut u64) -> u64 {
    let v = u8x64::from_slice_unaligned(data);

    let bs_bits = (v.eq(u8x64::splat(b'\\'))).bitmask();
    let escaped: u64;
    if bs_bits != 0 {
        escaped = get_escaped_branchless_u64(prev_escaped, bs_bits);
    } else {
        escaped = *prev_escaped;
        *prev_escaped = 0;
    }
    let quote_bits = (v.eq(u8x64::splat(b'"'))).bitmask() & !escaped;
    let in_string = prefix_xor(quote_bits) ^ *prev_instring;
    *prev_instring = (in_string as i64 >> 63) as u64;
    in_string
}

#[inline(always)]
fn skip_container_loop(
    input: &[u8; 64],
    prev_instring: &mut u64,
    prev_escaped: &mut u64,
    lbrace_num: &mut usize,
    rbrace_num: &mut usize,
    left: u8,
    right: u8,
) -> Option<NonZeroU8> {
    let instring = get_string_bits(input, prev_instring, prev_escaped);
    let v = u8x64::from_slice_unaligned(input);
    let last_lbrace_num = *lbrace_num;
    let mut rbrace = (v.eq(u8x64::splat(right))).bitmask() & !instring;
    let lbrace = (v.eq(u8x64::splat(left))).bitmask() & !instring;
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

/// A structure that used to get from json
pub struct Parser<R> {
    pub(crate) read: R,
    error_index: usize,   // mark the error position
    nospace_bits: u64,    // SIMD marked nospace bitmap
    nospace_start: isize, // the start position of nospace_bits
}

enum ParseStatus {
    None,
    HasEsacped,
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
        }
    }

    #[inline(always)]
    fn error_index(&self) -> usize {
        // when parsing strings , we need record the error postion.
        // it must be smaller than reader.index().
        std::cmp::min(self.error_index, self.read.index())
    }

    /// Error caused by a byte from next_char().
    #[cold]
    pub(crate) fn error(&self, reason: ErrorCode) -> Error {
        let position = self.read.position_of_index(self.error_index());
        Error::syntax(reason, position.line, position.column)
    }

    // maybe error in generated in visitor, so we need fix the postion.
    #[cold]
    pub(crate) fn fix_position(&self, err: Error) -> Error {
        err.fix_position(move |code| self.error(code))
    }

    #[inline(always)]
    pub(crate) fn parse_number(&mut self, negative: bool) -> Result<ParserNumber> {
        let reader = &mut self.read;
        let mut now = reader.index() - ((!negative) as usize);
        let data = reader.as_u8_slice();
        let ret = unsafe { parse_number_unchecked(data, &mut now, negative) };
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
        let ok = match self.parse_str(strbuf)? {
            Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
            Reference::Copied(s) => visitor.visit_str(s),
        };
        if !ok {
            perr!(self, UnexpectedVisitType)
        } else {
            Ok(())
        }
    }

    #[inline(always)]
    fn parse_string_inplace_visit<V>(&mut self, visitor: &mut V) -> Result<()>
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
    fn parse_number_visit<V>(&mut self, negative: bool, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let ok = match self.parse_number(negative)? {
            ParserNumber::Float(f) => visitor.visit_f64(f),
            ParserNumber::Unsigned(f) => visitor.visit_u64(f),
            ParserNumber::Signed(f) => visitor.visit_i64(f),
        };
        check_visit!(self, ok);
        Ok(())
    }

    #[inline(always)]
    fn parse_array<V>(&mut self, visitor: &mut V, strbuf: &mut Vec<u8>) -> Result<()>
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
                Some(b'-') => self.parse_number_visit(true, visitor),
                Some(b'0'..=b'9') => self.parse_number_visit(false, visitor),
                Some(b'"') => self.parse_string(visitor, strbuf),
                Some(b'{') => self.parse_object(visitor, strbuf),
                Some(b'[') => self.parse_array(visitor, strbuf),
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
        let ok = match self.parse_str(strbuf)? {
            Reference::Borrowed(s) => visitor.visit_borrowed_key(s),
            Reference::Copied(s) => visitor.visit_key(s),
        };
        check_visit!(self, ok);
        Ok(())
    }

    #[inline(always)]
    fn parse_object<V>(&mut self, visitor: &mut V, strbuf: &mut Vec<u8>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        // parseing empty object
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
            self.parse_key(visitor, strbuf)?;
            self.parse_object_clo()?;
            self.parse_value(visitor, strbuf)?;
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
        if let Some(chunck) = reader.next_n(literal.len()) {
            if chunck != literal.as_bytes() {
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

    // parse single json raw value, use simd skip.
    #[inline]
    fn parse_elem_lazy(&mut self) -> Result<(&'de [u8], JsonType)> {
        let typ = self.get_json_type()?;
        let raw = self.skip_one()?;
        Ok((raw, typ))
    }

    #[inline]
    pub(crate) fn parse_array_elem_lazy(
        &mut self,
        first: &mut bool,
    ) -> Result<Option<(&'de [u8], JsonType)>> {
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
        let ret = self.parse_elem_lazy()?;
        Ok(Some(ret))
    }

    #[inline]
    pub(crate) fn parse_entry_lazy(
        &mut self,
        strbuf: &mut Vec<u8>,
        first: &mut bool,
    ) -> Result<Option<(FastStr, &'de [u8], JsonType)>> {
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

        let key = match self.parse_str(strbuf)? {
            Reference::Borrowed(s) => FastStr::new(s),
            Reference::Copied(s) => FastStr::new(s),
        };
        self.parse_object_clo()?;
        let typ = self.get_json_type()?;
        let raw = self.skip_one()?;
        Ok(Some((key, raw, typ)))
    }

    fn parse_value<V>(&mut self, visitor: &mut V, strbuf: &mut Vec<u8>) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        match self.skip_space() {
            Some(b'-') => self.parse_number_visit(true, visitor),
            Some(b'0'..=b'9') => self.parse_number_visit(false, visitor),
            Some(b'"') => self.parse_string(visitor, strbuf),
            Some(b'{') => self.parse_object(visitor, strbuf),
            Some(b'[') => self.parse_array(visitor, strbuf),
            Some(first) => self.parse_literal_visit(first, visitor),
            None => perr!(self, EofWhileParsing),
        }
    }

    #[cold]
    fn parse_single<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        let r = &mut self.read;
        if r.index() == 0 {
            return perr!(self, EofWhileParsing);
        }
        match r.at(r.index() - 1) {
            b'-' => self.parse_number_visit(true, visitor),
            b'0'..=b'9' => self.parse_number_visit(false, visitor),
            b'"' => self.parse_string_inplace_visit(visitor),
            first => self.parse_literal_visit(first, visitor),
        }
    }

    pub(crate) fn parse_value_goto<V>(&mut self, visitor: &mut V) -> Result<()>
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
            _ => return self.parse_single(visitor),
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
                            b'0'..=b'9' => self.parse_number_visit(false, visitor)?,
                            b'-' => self.parse_number_visit(true, visitor)?,
                            b'"' => self.parse_string_inplace_visit(visitor)?,
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
                        self.parse_string_inplace_visit(visitor)?;
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
                            b'0'..=b'9' => self.parse_number_visit(false, visitor)?,
                            b'-' => self.parse_number_visit(true, visitor)?,
                            b'"' => self.parse_string_inplace_visit(visitor)?,
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
                            // Note: we will not check trailing charaters
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
    pub(crate) fn parse_str<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<Reference<'de, 'own, str>> {
        let slice = self.parse_string_raw(buf)?;
        Ok(match slice {
            Reference::Copied(buf) => Reference::Copied(unsafe { from_utf8_unchecked(buf) }),
            Reference::Borrowed(buf) => Reference::Borrowed(unsafe { from_utf8_unchecked(buf) }),
        })
    }

    #[inline(always)]
    // parse_string_raw maybe borrowed, maybe copied into buf(buf will be clear at first).
    pub(crate) fn parse_string_raw<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<Reference<'de, 'own, [u8]>> {
        // now reader is start after `"`, so we can directly skipstring
        let start = self.read.index();
        let status = self.skip_string_impl()?;
        let key = self.read.slice_unchecked(start, self.read.index() - 1);
        match status {
            ParseStatus::HasEsacped => {
                buf.clear();
                match parse_valid_escaped_string(key, buf) {
                    Ok(_) => Ok(Reference::Copied(buf)),
                    Err(code) => {
                        self.error_index = start;
                        perr!(self, code)
                    }
                }
            }
            _ => Ok(Reference::Borrowed(key)),
        }
    }

    #[inline(always)]
    fn get_next_token<const N: usize>(&mut self, tokens: [u8; N], advance: usize) -> Option<u8> {
        let r = &mut self.read;
        const LANS: usize = u8x32::lanes();
        while let Some(chunck) = r.peek_n(LANS) {
            let v = unsafe { u8x32::from_slice_unaligned_unchecked(chunck) };
            let mut vor = m8x32::splat(false);
            for t in tokens.iter().take(N) {
                vor |= v.eq(u8x32::splat(*t));
            }
            let next = vor.bitmask();
            if next != 0 {
                let cnt = next.trailing_zeros() as usize;
                let ch = chunck[cnt];
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

    // skip_string skips a JSON string, and return the later parts afer closed quote, and the escaped status.
    // skip_string always start with the quote marks.
    #[inline(always)]
    fn skip_string_impl(&mut self) -> Result<ParseStatus> {
        const LANS: usize = u8x32::lanes();
        let r = &mut self.read;
        let mut quote_bits;
        let mut escaped;
        let mut prev_escaped = 0;
        let mut status = ParseStatus::None;

        while let Some(chunck) = r.peek_n(LANS) {
            let v = unsafe { u8x32::from_slice_unaligned_unchecked(chunck) };
            let bs_bits = (v.eq(u8x32::splat(b'\\'))).bitmask();
            quote_bits = (v.eq(u8x32::splat(b'"'))).bitmask();
            //
            // maybe has escaped quotes
            if ((quote_bits.wrapping_sub(1)) & bs_bits) != 0 || prev_escaped != 0 {
                escaped = get_escaped_branchless_u32(&mut prev_escaped, bs_bits);
                status = ParseStatus::HasEsacped;
                //
                quote_bits &= !escaped;
            }
            //
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
                status = ParseStatus::HasEsacped;
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
    fn skip_string(&mut self) -> Result<()> {
        let _ = self.skip_string_impl()?;
        // ignore the status of hasesacped
        Ok(())
    }

    // parse the Colon :
    #[inline(always)]
    pub(crate) fn parse_object_clo(&mut self) -> Result<()> {
        match self.skip_space() {
            Some(b':') => Ok(()),
            Some(_) => perr!(self, ExpectedColon),
            None => perr!(self, EofWhileParsing),
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

    /// skip_container skip a object or array, and retu
    #[inline(always)]
    fn skip_container(&mut self, left: u8, right: u8) -> Result<()> {
        let mut prev_instring = 0;
        let mut prev_escaped = 0;
        let mut rbrace_num = 0;
        let mut lbrace_num = 0;
        let reader = &mut self.read;

        while let Some(chunck) = reader.peek_n(64) {
            let input = array_ref![chunck, 0, 64];
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
            let n = reader.remain() as usize;
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

    // TODO: add nospace bitmap optimize
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
        while let Some(chunck) = reader.peek_n(64) {
            let chunck = array_ref![chunck, 0, 64];
            let bitmap = get_nonspace_bits(chunck);
            if bitmap != 0 {
                self.nospace_bits = bitmap;
                self.nospace_start = reader.index() as isize;
                let cnt = bitmap.trailing_zeros() as usize;
                let ch = chunck[cnt];
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
        while let Some(chunck) = reader.peek_n(64) {
            let chunck = array_ref![chunck, 0, 64];
            let bitmap = get_nonspace_bits(chunck);
            if bitmap != 0 {
                self.nospace_bits = bitmap;
                self.nospace_start = reader.index() as isize;
                let cnt = bitmap.trailing_zeros() as usize;
                let ch = chunck[cnt];
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
        if let Some(chunck) = reader.next_n(literal.len()) {
            if chunck == literal.as_bytes() {
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
    pub(crate) fn skip_one(&mut self) -> Result<&'de [u8]> {
        let ch = self.skip_space();
        let start = self.read.index() - 1;
        match ch {
            Some(b'-' | b'0'..=b'9') => self.skip_number_unsafe(),
            Some(b'"') => self.skip_string(),
            Some(b'{') => self.skip_container(b'{', b'}'),
            Some(b'[') => self.skip_container(b'[', b']'),
            Some(b't') => self.parse_literal("rue"),
            Some(b'f') => self.parse_literal("alse"),
            Some(b'n') => self.parse_literal("ull"),
            Some(_) => perr!(self, InvalidJsonValue),
            None => perr!(self, EofWhileParsing),
        }?;
        let slice = self.read.slice_unchecked(start, self.read.index());
        Ok(slice)
    }

    #[inline(always)]
    pub(crate) fn parse_trailing(&mut self) -> Result<()> {
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
        // we assume parsed_key has always
        debug_assert!(temp_buf.is_empty());
        match self.skip_space() {
            Some(b'{') => {}
            Some(_) => return perr!(self, ExpectedObjectStart),
            None => return perr!(self, EofWhileParsing),
        }

        // deal with the empty object
        match self.get_next_token([b'"', b'}'], 1) {
            Some(b'"') => {}
            Some(b'}') => return perr!(self, GetInEmptyObj),
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
                Some(b'"') => self.skip_string()?,
                None => return perr!(self, EofWhileParsing),
                _ => {}
            };

            // optimze: direct find the next quote of key. or object ending
            match self.get_next_token([b'"', b'}'], 1) {
                Some(b'"') => continue,
                Some(b'}') => return perr!(self, GetUnknownKeyInObj),
                None => return perr!(self, EofWhileParsing),
                Some(_) => unreachable!(),
            }
        }
    }

    // get_from_array will make reader at the position after target index in JSON array.
    #[inline(always)]
    fn get_from_array(&mut self, index: usize) -> Result<()> {
        let mut count = index;
        match self.skip_space() {
            Some(b'[') => {}
            Some(_) => return perr!(self, ExpectedArrayStart),
            None => return perr!(self, EofWhileParsing),
        }
        while count > 0 {
            // skip object,array,string at first
            match self.skip_space() {
                Some(b'{') => self.skip_container(b'{', b'}')?,
                Some(b'[') => self.skip_container(b'[', b']')?,
                Some(b'"') => self.skip_string()?,
                Some(b']') => return perr!(self, GetIndexOutOfArray),
                None => return perr!(self, EofWhileParsing),
                _ => {}
            };

            // optimze: direct find the next token
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

    pub(crate) fn get_from(&mut self, path: &JsonPointer) -> Result<&'de [u8]> {
        self.get_from_with_iter(path.iter())
    }

    pub(crate) fn get_from_with_iter<Iter: Iterator>(&mut self, iter: Iter) -> Result<&'de [u8]>
    where
        Iter::Item: PointerTrait,
    {
        // temp buf reused when parsing each escaped key
        let mut temp_buf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        for jp in iter {
            if let Some(key) = jp.key() {
                self.get_from_object(key, &mut temp_buf)
            } else if let Some(index) = jp.index() {
                self.get_from_array(index)
            } else {
                unreachable!();
            }?;
        }
        // TODO: optimize not need skip the latest field. return the remain JSON.
        let slice = self.skip_one()?;
        Ok(slice)
    }

    fn get_many_rec(
        &mut self,
        node: &PointerTreeNode,
        out: &mut Vec<&'de [u8]>,
        strbuf: &mut Vec<u8>,
        remain: &mut usize,
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

        match &node.children {
            PointerTreeInner::Empty => {
                self.skip_one()?;
            }
            PointerTreeInner::Index(midxs) => self.get_many_index(midxs, strbuf, out, remain)?,
            PointerTreeInner::Key(mkeys) => self.get_many_keys(mkeys, strbuf, out, remain)?,
        };

        if !node.order.is_empty() {
            slice = self.read.slice_unchecked(start, self.read.index());
            for p in &node.order {
                out[*p] = slice;
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
        out: &mut Vec<&'de [u8]>,
        remain: &mut usize,
    ) -> Result<()> {
        debug_assert!(strbuf.is_empty());
        match self.skip_space() {
            Some(b'{') => {}
            Some(_) => return perr!(self, ExpectedObjectStart),
            None => return perr!(self, EofWhileParsing),
        }

        // deal with the empty object
        match self.get_next_token([b'"', b'}'], 1) {
            Some(b'"') => {}
            Some(b'}') => return perr!(self, GetInEmptyObj),
            None => return perr!(self, EofWhileParsing),
            Some(_) => unreachable!(),
        }

        let mut visited = 0;
        loop {
            let key = self.parse_str(strbuf)?;
            self.parse_object_clo()?;
            if let Some(val) = mkeys.get(key.deref()) {
                self.get_many_rec(val, out, strbuf, remain)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else {
                // skip object,array,string at first
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => self.skip_string()?,
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
            }

            // optimze: direct find the next quote of key. or object ending
            match self.get_next_token([b'"', b'}'], 1) {
                Some(b'"') => {}
                Some(b'}') => break,
                None => return perr!(self, EofWhileParsing),
                Some(_) => unreachable!(),
            }
        }

        // check wheter remaining unknown keys
        if visited < mkeys.len() {
            perr!(self, GetUnknownKeyInObj)
        } else {
            Ok(())
        }
    }

    pub(crate) fn remain_str(&self) -> &'de str {
        as_str(self.remain_u8_slice())
    }

    pub(crate) fn remain_u8_slice(&self) -> &'de [u8] {
        let reader = &self.read;
        let len = reader.remain() as usize;
        let start = reader.index();
        reader.slice_unchecked(start, start + len)
    }

    fn get_many_index(
        &mut self,
        midx: &MultiIndex,
        strbuf: &mut Vec<u8>,
        out: &mut Vec<&'de [u8]>,
        remain: &mut usize,
    ) -> Result<()> {
        match self.skip_space() {
            Some(b'[') => {}
            Some(_) => return perr!(self, ExpectedArrayStart),
            None => return perr!(self, EofWhileParsing),
        }
        let mut index = 0;
        let mut visited = 0;
        loop {
            if let Some(val) = midx.get(&index) {
                self.get_many_rec(val, out, strbuf, remain)?;
                visited += 1;
                if *remain == 0 {
                    break;
                }
            } else {
                // skip object,array,string at first
                match self.skip_space() {
                    Some(b'{') => self.skip_container(b'{', b'}')?,
                    Some(b'[') => self.skip_container(b'[', b']')?,
                    Some(b'"') => self.skip_string()?,
                    None => return perr!(self, EofWhileParsing),
                    _ => {}
                };
            }

            // optimze: direct find the next token
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

        // check wheter remaining unknown keys
        if visited < midx.len() {
            perr!(self, GetIndexOutOfArray)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_many(&mut self, tree: &PointerTree) -> Result<Vec<&'de [u8]>> {
        let mut strbuf = Vec::with_capacity(DEFAULT_KEY_BUF_CAPACITY);
        let mut remain = tree.count();
        let mut out: Vec<&'de [u8]> = Vec::with_capacity(tree.count());
        for _i in 0..tree.count() {
            out.push(&[])
        }
        let cur = &tree.root;
        self.get_many_rec(cur, &mut out, &mut strbuf, &mut remain)?;
        Ok(out)
    }
}
