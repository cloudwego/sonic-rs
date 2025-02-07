use std::{
    mem::MaybeUninit,
    slice::{from_raw_parts, from_raw_parts_mut},
    str::from_utf8_unchecked,
};

#[cfg(not(all(target_feature = "neon", target_arch = "aarch64")))]
use sonic_simd::u8x32;
#[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
use sonic_simd::{bits::NeonBits, u8x16};
use sonic_simd::{BitMask, Mask, Simd};

use crate::{
    error::ErrorCode::{
        self, ControlCharacterWhileParsingString, InvalidEscape, InvalidUnicodeCodePoint,
    },
    util::unicode::handle_unicode_codepoint_mut,
};

#[inline(always)]
pub unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    from_utf8_unchecked(from_raw_parts(ptr, len))
}

pub const ESCAPED_TAB: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, b'"', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b'/', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    b'\\', 0, 0, 0, 0, 0, b'\x08', /* \b */
    0, 0, 0, b'\x0c', /* \f */
    0, 0, 0, 0, 0, 0, 0, b'\n', 0, 0, 0, b'\r', 0, b'\t', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[derive(Debug)]
pub(crate) struct StringBlock<B: BitMask> {
    pub(crate) bs_bits: B,
    pub(crate) quote_bits: B,
    pub(crate) unescaped_bits: B,
}

#[cfg(not(all(target_feature = "neon", target_arch = "aarch64")))]
impl StringBlock<u32> {
    pub(crate) const LANES: usize = 32;

    #[inline]
    pub fn new(v: &u8x32) -> Self {
        Self {
            bs_bits: (v.eq(&u8x32::splat(b'\\'))).bitmask(),
            quote_bits: (v.eq(&u8x32::splat(b'"'))).bitmask(),
            unescaped_bits: (v.le(&u8x32::splat(0x1f))).bitmask(),
        }
    }
}

#[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
impl StringBlock<NeonBits> {
    pub(crate) const LANES: usize = 16;

    #[inline]
    pub fn new(v: &u8x16) -> Self {
        Self {
            bs_bits: (v.eq(&u8x16::splat(b'\\'))).bitmask(),
            quote_bits: (v.eq(&u8x16::splat(b'"'))).bitmask(),
            unescaped_bits: (v.le(&u8x16::splat(0x1f))).bitmask(),
        }
    }
}

impl<B: BitMask> StringBlock<B> {
    #[inline(always)]
    pub fn has_unescaped(&self) -> bool {
        self.unescaped_bits.before(&self.quote_bits)
    }

    #[inline(always)]
    pub fn has_quote_first(&self) -> bool {
        self.quote_bits.before(&self.bs_bits) && !self.has_unescaped()
    }

    #[inline(always)]
    pub fn has_backslash(&self) -> bool {
        self.bs_bits.before(&self.quote_bits)
    }

    #[inline(always)]
    pub fn quote_index(&self) -> usize {
        self.quote_bits.first_offset()
    }

    #[inline(always)]
    pub fn bs_index(&self) -> usize {
        self.bs_bits.first_offset()
    }

    #[inline(always)]
    pub fn unescaped_index(&self) -> usize {
        self.unescaped_bits.first_offset()
    }
}

#[inline(always)]
pub(crate) unsafe fn load<V: Simd>(ptr: *const u8) -> V {
    let chunk = from_raw_parts(ptr, V::LANES);
    V::from_slice_unaligned_unchecked(chunk)
}

/// Return the size of the actual parsed string, `repr` means repr invalid UTF16 surrogate with
/// `\uFFFD`
/// TODO: fix me, there are repeat codes!!!
#[inline(always)]
pub(crate) unsafe fn parse_string_inplace(
    src: &mut *mut u8,
    repr: bool,
) -> std::result::Result<usize, ErrorCode> {
    #[cfg(all(target_feature = "neon", target_arch = "aarch64"))]
    let mut block: StringBlock<NeonBits>;
    #[cfg(not(all(target_feature = "neon", target_arch = "aarch64")))]
    let mut block: StringBlock<u32>;

    let sdst = *src;
    let src: &mut *const u8 = std::mem::transmute(src);

    // loop for string without escaped chars
    loop {
        block = StringBlock::new(&unsafe { load(*src) });
        if block.has_quote_first() {
            let idx = block.quote_index();
            *src = src.add(idx + 1);
            return Ok(src.offset_from(sdst) as usize - 1);
        }
        if block.has_unescaped() {
            return Err(ControlCharacterWhileParsingString);
        }
        if block.has_backslash() {
            break;
        }
        *src = src.add(StringBlock::LANES);
    }

    let bs_dist = block.bs_index();
    *src = src.add(bs_dist);
    let mut dst = sdst.add((*src as usize) - sdst as usize);

    // loop for string with escaped chars
    loop {
        'escape: loop {
            let escaped_char: u8 = *src.add(1);
            if escaped_char == b'u' {
                if !handle_unicode_codepoint_mut(src, &mut dst, repr) {
                    return Err(InvalidUnicodeCodePoint);
                }
            } else {
                *dst = ESCAPED_TAB[escaped_char as usize];
                if *dst == 0 {
                    return Err(InvalidEscape);
                }
                *src = src.add(2);
                dst = dst.add(1);
            }

            // fast path for continuous escaped chars
            if **src == b'\\' {
                continue 'escape;
            }
            break 'escape;
        }

        'find_and_move: loop {
            let v = unsafe { load(*src) };
            let block = StringBlock::new(&v);
            if block.has_quote_first() {
                while **src != b'"' {
                    *dst = **src;
                    dst = dst.add(1);
                    *src = src.add(1);
                }
                *src = src.add(1); // skip ending quote
                return Ok(dst.offset_from(sdst) as usize);
            }
            if block.has_unescaped() {
                return Err(ControlCharacterWhileParsingString);
            }
            if !block.has_backslash() {
                let chunk = from_raw_parts_mut(dst, StringBlock::LANES);
                v.write_to_slice_unaligned_unchecked(chunk);
                *src = src.add(StringBlock::LANES);
                dst = dst.add(StringBlock::LANES);
                continue 'find_and_move;
            }
            // TODO: loop unrooling here
            while **src != b'\\' {
                *dst = **src;
                dst = dst.add(1);
                *src = src.add(1);
            }
            break 'find_and_move;
        }
    } // slow loop for escaped chars
}

pub const QUOTE_TAB: [(u8, [u8; 8]); 256] = [
    // 0x00 ~ 0x1f
    (6, *b"\\u0000\0\0"),
    (6, *b"\\u0001\0\0"),
    (6, *b"\\u0002\0\0"),
    (6, *b"\\u0003\0\0"),
    (6, *b"\\u0004\0\0"),
    (6, *b"\\u0005\0\0"),
    (6, *b"\\u0006\0\0"),
    (6, *b"\\u0007\0\0"),
    (2, *b"\\b\0\0\0\0\0\0"),
    (2, *b"\\t\0\0\0\0\0\0"),
    (2, *b"\\n\0\0\0\0\0\0"),
    (6, *b"\\u000b\0\0"),
    (2, *b"\\f\0\0\0\0\0\0"),
    (2, *b"\\r\0\0\0\0\0\0"),
    (6, *b"\\u000e\0\0"),
    (6, *b"\\u000f\0\0"),
    (6, *b"\\u0010\0\0"),
    (6, *b"\\u0011\0\0"),
    (6, *b"\\u0012\0\0"),
    (6, *b"\\u0013\0\0"),
    (6, *b"\\u0014\0\0"),
    (6, *b"\\u0015\0\0"),
    (6, *b"\\u0016\0\0"),
    (6, *b"\\u0017\0\0"),
    (6, *b"\\u0018\0\0"),
    (6, *b"\\u0019\0\0"),
    (6, *b"\\u001a\0\0"),
    (6, *b"\\u001b\0\0"),
    (6, *b"\\u001c\0\0"),
    (6, *b"\\u001d\0\0"),
    (6, *b"\\u001e\0\0"),
    (6, *b"\\u001f\0\0"),
    // 0x20 ~ 0x2f
    (0, [0; 8]),
    (0, [0; 8]),
    (2, *b"\\\"\0\0\0\0\0\0"),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x30 ~ 0x3f
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x40 ~ 0x4f
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x50 ~ 0x5f
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (2, *b"\\\\\0\0\0\0\0\0"),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x60 ~ 0xff
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
];

const NEED_ESCAPED: [u8; 256] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// only check the src length.
#[inline(always)]
unsafe fn escape_unchecked(src: &mut *const u8, nb: &mut usize, dst: &mut *mut u8) {
    assert!(*nb >= 1);
    loop {
        let ch = *(*src);
        let cnt = QUOTE_TAB[ch as usize].0 as usize;
        assert!(
            cnt != 0,
            "char is {}, cnt is {},  NEED_ESCAPED is {}",
            ch as char,
            cnt,
            NEED_ESCAPED[ch as usize]
        );
        std::ptr::copy_nonoverlapping(QUOTE_TAB[ch as usize].1.as_ptr(), *dst, 8);
        (*dst) = (*dst).add(cnt);
        (*src) = (*src).add(1);
        (*nb) -= 1;
        if (*nb) == 0 || NEED_ESCAPED[*(*src) as usize] == 0 {
            return;
        }
    }
}

#[inline(always)]
fn check_cross_page(ptr: *const u8, step: usize) -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let page_size = 4096;
        ((ptr as usize & (page_size - 1)) + step) > page_size
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // not check page cross in fallback envs, always true
        true
    }
}

#[inline(always)]
pub fn format_string(value: &str, dst: &mut [MaybeUninit<u8>], need_quote: bool) -> usize {
    assert!(dst.len() >= value.len() * 6 + 32 + 3);

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let mut v: u8x16;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let mut v: u8x32;

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    const LANES: usize = 16;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    const LANES: usize = 32;

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[inline]
    fn escaped_mask(v: u8x16) -> NeonBits {
        let x1f = u8x16::splat(0x1f); // 0x00 ~ 0x20
        let blash = u8x16::splat(b'\\');
        let quote = u8x16::splat(b'"');
        let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
        v.bitmask()
    }

    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    #[inline]
    fn escaped_mask(v: u8x32) -> u32 {
        let x1f = u8x32::splat(0x1f); // 0x00 ~ 0x20
        let blash = u8x32::splat(b'\\');
        let quote = u8x32::splat(b'"');
        let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
        v.bitmask()
    }

    unsafe {
        let slice = value.as_bytes();
        let mut sptr = slice.as_ptr();
        let mut dptr = dst.as_mut_ptr() as *mut u8;
        let dstart = dptr;
        let mut nb: usize = slice.len();

        if need_quote {
            *dptr = b'"';
            dptr = dptr.add(1);
        }
        while nb >= LANES {
            v = load(sptr);
            v.write_to_slice_unaligned_unchecked(std::slice::from_raw_parts_mut(dptr, LANES));
            let mask = escaped_mask(v);
            if mask.all_zero() {
                nb -= LANES;
                dptr = dptr.add(LANES);
                sptr = sptr.add(LANES);
            } else {
                let cn = mask.first_offset();
                nb -= cn;
                dptr = dptr.add(cn);
                sptr = sptr.add(cn);
                escape_unchecked(&mut sptr, &mut nb, &mut dptr);
            }
        }

        let mut temp: [u8; LANES] = [0u8; LANES];
        while nb > 0 {
            v = if check_cross_page(sptr, LANES) {
                std::ptr::copy_nonoverlapping(sptr, temp[..].as_mut_ptr(), nb);
                load(temp[..].as_ptr())
            } else {
                #[cfg(not(any(debug_assertions, feature = "sanitize")))]
                {
                    // disable memory sanitizer here
                    load(sptr)
                }
                #[cfg(any(debug_assertions, feature = "sanitize"))]
                {
                    std::ptr::copy_nonoverlapping(sptr, temp[..].as_mut_ptr(), nb);
                    load(temp[..].as_ptr())
                }
            };
            v.write_to_slice_unaligned_unchecked(std::slice::from_raw_parts_mut(dptr, LANES));

            let mask = escaped_mask(v).clear_high_bits(LANES - nb);
            if mask.all_zero() {
                dptr = dptr.add(nb);
                break;
            } else {
                let cn = mask.first_offset();
                nb -= cn;
                dptr = dptr.add(cn);
                sptr = sptr.add(cn);
                escape_unchecked(&mut sptr, &mut nb, &mut dptr);
            }
        }
        if need_quote {
            *dptr = b'"';
            dptr = dptr.add(1);
        }
        dptr as usize - dstart as usize
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_quote() {
        let mut dst = [0u8; 1000];
        let dst_ref = unsafe { std::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(&mut dst) };
        assert_eq!(format_string("", dst_ref, true), 2);
        assert_eq!(dst[..2], *b"\"\"");
        assert_eq!(format_string("\x00", dst_ref, true), 8);
        assert_eq!(dst[..8], *b"\"\\u0000\"");
        assert_eq!(format_string("test", dst_ref, true), 6);
        assert_eq!(dst[..6], *b"\"test\"");
        assert_eq!(format_string("test\"test", dst_ref, true), 12);
        assert_eq!(dst[..12], *b"\"test\\\"test\"");
        assert_eq!(format_string("\\testtest\"", dst_ref, true), 14);
        assert_eq!(dst[..14], *b"\"\\\\testtest\\\"\"");

        let long_str = "this is a long string that should be \\\"quoted and escaped multiple \
                        times to test the performance and correctness of the function.";
        assert_eq!(format_string(long_str, dst_ref, true), 129 + 4);
        assert_eq!(dst[..133], *b"\"this is a long string that should be \\\\\\\"quoted and escaped multiple times to test the performance and correctness of the function.\"");
    }
}
