use std::{mem::MaybeUninit, slice::from_raw_parts, str::from_utf8_unchecked};

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

const fn build_escaped_tab() -> [u8; 256] {
    let mut arr = [0u8; 256];
    arr[b'"' as usize] = b'"';
    arr[b'/' as usize] = b'/';
    arr[b'\\' as usize] = b'\\';
    arr[b'b' as usize] = 0x08;
    arr[b'f' as usize] = 0x0c;
    arr[b'n' as usize] = 0x0a;
    arr[b'r' as usize] = 0x0d;
    arr[b't' as usize] = 0x09;
    arr
}

pub const ESCAPED_TAB: [u8; 256] = build_escaped_tab();

#[derive(Debug)]
pub(crate) struct StringBlock<B: BitMask> {
    pub(crate) bs_bits: B,
    pub(crate) quote_bits: B,
    pub(crate) unescaped_bits: B,
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

impl StringBlock<u32> {
    #[allow(unused)]
    #[inline(always)]
    pub(crate) fn new(v: &sonic_simd::u8x32) -> Self {
        let v_bs = v.eq(&sonic_simd::u8x32::splat(b'\\'));
        let v_quote = v.eq(&sonic_simd::u8x32::splat(b'"'));
        let v_cc = v.le(&sonic_simd::u8x32::splat(0x1f));
        Self {
            bs_bits: v_bs.bitmask(),
            quote_bits: v_quote.bitmask(),
            unescaped_bits: v_cc.bitmask(),
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
impl StringBlock<sonic_simd::bits::NeonBits> {
    #[allow(unused)]
    #[inline(always)]
    pub(crate) fn new(v: &sonic_simd::u8x16) -> Self {
        use sonic_simd::u8x16;
        let v_bs = v.eq(&u8x16::splat(b'\\'));
        let v_quote = v.eq(&u8x16::splat(b'"'));
        let v_cc = v.le(&u8x16::splat(0x1f));
        Self {
            bs_bits: v_bs.bitmask(),
            quote_bits: v_quote.bitmask(),
            unescaped_bits: v_cc.bitmask(),
        }
    }
}

#[inline(always)]
pub(crate) unsafe fn load<V: Simd>(ptr: *const u8) -> V {
    let chunk = from_raw_parts(ptr, V::LANES);
    V::from_slice_unaligned_unchecked(chunk)
}

// build_block is defined per-arch via cfg_if below
// Detect SVE2 first, then Neon, then fallback
cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "aarch64", target_feature = "sve2"))] {
        use sonic_simd::bits::SveBits;
        use sonic_simd::u8x16;

        #[inline(always)]
        pub(crate) fn build_block(ptr: &u8x16) -> StringBlock<SveBits> {
            let (q, bs, un): (u64, u64, u64);
            unsafe {
                core::arch::asm!(
                    "ptrue p0.b, vl16",
                    "ld1b {{z0.b}}, p0/z, [{ptr}]",

                    // '"'
                    "mov z1.b, #34",
                    "match p1.b, p0/z, z0.b, z1.b",
                    "brkb  p1.b, p0/z, p1.b",
                    "cntp  {q_idx}, p0, p1.b",

                    // '\\'
                    "mov z1.b, #92",
                    "match p1.b, p0/z, z0.b, z1.b",
                    "brkb  p1.b, p0/z, p1.b",
                    "cntp  {bs_idx}, p0, p1.b",

                    // ascii control characters (<= 0x1f) using unsigned compare
                    "mov z1.b, #31",
                    "cmpls p1.b, p0/z, z0.b, z1.b",
                    "brkb  p1.b, p0/z, p1.b",
                    "cntp  {un_idx}, p0, p1.b",

                    ptr = in(reg) ptr,
                    q_idx = out(reg) q,
                    bs_idx = out(reg) bs,
                    un_idx = out(reg) un,
                    out("z0") _, out("z1") _,
                    out("p0") _, out("p1") _,
                );
            }
            StringBlock {
                quote_bits: SveBits::new(q as usize),
                bs_bits: SveBits::new(bs as usize),
                unescaped_bits: SveBits::new(un as usize),
            }
        }

        pub(crate) fn load_v(ptr: *const u8) -> u8x16 {
            unsafe { load::<u8x16>(ptr) }
        }

        pub const STRING_BLOCK_LANES: usize = 16;
    } else if #[cfg(all(target_arch = "aarch64", target_feature = "neon"))] {
        use sonic_simd::{bits::NeonBits, u8x16};

        pub(crate) fn load_v(ptr: *const u8) -> u8x16 {
            unsafe { load::<u8x16>(ptr) }
        }

        #[inline(always)]
        pub(crate) fn build_block(v: &u8x16) -> StringBlock<NeonBits> {
            StringBlock::<NeonBits>::new(v)
        }

        pub const STRING_BLOCK_LANES: usize = 16;
    } else {
        use sonic_simd::u8x32;

        pub(crate) fn load_v(ptr: *const u8) -> u8x32 {
            unsafe { load::<u8x32>(ptr) }
        }

        #[inline(always)]
        pub(crate) fn build_block(v: &u8x32) -> StringBlock<u32> {
            StringBlock::<u32>::new(v)
        }

        pub const STRING_BLOCK_LANES: usize = 32;
    }
}

/// Return the size of the actual parsed string, `repr` means repr invalid UTF16 surrogate with
/// `\uFFFD`
/// TODO: fix me, there are repeat codes!!!
#[inline(always)]
pub(crate) unsafe fn parse_string_inplace(
    src: &mut *mut u8,
    repr: bool,
) -> std::result::Result<usize, ErrorCode> {
    let sdst = *src;
    let src: &mut *const u8 = std::mem::transmute(src);

    // loop for string without escaped chars (original control flow)
    let mut v = load_v(*src);
    let mut block = build_block(&v);
    loop {
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
        *src = src.add(STRING_BLOCK_LANES);
        v = load_v(*src);
        block = build_block(&v);
    }

    let bs_dist = block.bs_index();
    *src = src.add(bs_dist);
    let mut dst = sdst.add((*src as usize) - sdst as usize);

    // loop for string with escaped chars (original control flow)
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
            let v = load_v(*src);
            let block = build_block(&v);
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
                // copy a full chunk without escapes using SIMD store
                let chunk = std::slice::from_raw_parts_mut(dst, STRING_BLOCK_LANES);
                v.write_to_slice_unaligned_unchecked(chunk);
                *src = src.add(STRING_BLOCK_LANES);
                dst = dst.add(STRING_BLOCK_LANES);
                continue 'find_and_move;
            }
            // TODO: loop unrolling here
            while **src != b'\\' {
                *dst = **src;
                dst = dst.add(1);
                *src = src.add(1);
            }
            break 'find_and_move;
        }
    } // slow loop for escaped chars
}

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
        true
    }
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

#[inline(always)]
pub fn format_string(value: &str, dst: &mut [MaybeUninit<u8>], need_quote: bool) -> usize {
    assert!(dst.len() >= value.len() * 6 + 32 + 3);

    cfg_if::cfg_if! {
        if #[cfg(all(target_arch = "aarch64", target_feature = "sve2"))] {
            use sonic_simd::{bits::SveBits, u8x16};
            let mut v: u8x16;
            const LANES: usize = 16;

            #[inline(always)]
            fn escaped_mask_at(ptr: *const u8) -> SveBits {
                let (q, bs, un): (u64, u64, u64);
                unsafe {
                    core::arch::asm!(
                        "ptrue p0.b, vl16",
                        "ld1b {{z0.b}}, p0/z, [{ptr}]",

                        // '"'
                        "mov z1.b, #34",
                        "match p1.b, p0/z, z0.b, z1.b",
                        "brkb  p1.b, p0/z, p1.b",
                        "cntp  {q_idx}, p0, p1.b",

                        // '\\'
                        "mov z1.b, #92",
                        "match p1.b, p0/z, z0.b, z1.b",
                        "brkb  p1.b, p0/z, p1.b",
                        "cntp  {bs_idx}, p0, p1.b",

                        // ascii control characters (<= 0x1f)
                        "mov z1.b, #31",
                        "cmpls p1.b, p0/z, z0.b, z1.b",
                        "brkb  p1.b, p0/z, p1.b",
                        "cntp  {un_idx}, p0, p1.b",

                        ptr = in(reg) ptr,
                        q_idx = out(reg) q,
                        bs_idx = out(reg) bs,
                        un_idx = out(reg) un,
                        out("z0") _, out("z1") _,
                        out("p0") _, out("p1") _,
                    );
                }
                let idx = core::cmp::min(q, core::cmp::min(bs, un)) as usize;
                SveBits::new(idx)
            }
        } else if #[cfg(all(target_arch = "aarch64", target_feature = "neon"))] {
            use sonic_simd::{bits::NeonBits, u8x16};
            let mut v: u8x16;
            const LANES: usize = 16;

            #[inline(always)]
            fn escaped_mask(v: u8x16) -> NeonBits {
                let x1f = u8x16::splat(0x1f);
                let blash = u8x16::splat(b'\\');
                let quote = u8x16::splat(b'\"');
                let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
                v.bitmask()
            }
        } else {
            use sonic_simd::u8x32;
            let mut v: u8x32;
            const LANES: usize = 32;

            #[inline(always)]
            fn escaped_mask(v: u8x32) -> u32 {
                let x1f = u8x32::splat(0x1f);
                let blash = u8x32::splat(b'\\');
                let quote = u8x32::splat(b'\"');
                let v = v.le(&x1f) | v.eq(&blash) | v.eq(&quote);
                v.bitmask()
            }
        }
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
            v = load_v(sptr);
            v.write_to_slice_unaligned_unchecked(std::slice::from_raw_parts_mut(dptr, LANES));
            cfg_if::cfg_if! {
                if #[cfg(all(target_arch = "aarch64", target_feature = "sve2"))] {
                    let mask = escaped_mask_at(sptr);
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
                } else {
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
            }
        }

        let mut temp: [u8; LANES] = [0u8; LANES];
        while nb > 0 {
            v = if check_cross_page(sptr, LANES) {
                std::ptr::copy_nonoverlapping(sptr, temp[..].as_mut_ptr(), nb);
                load_v(temp[..].as_ptr())
            } else {
                #[cfg(not(any(debug_assertions, feature = "sanitize")))]
                {
                    load_v(sptr)
                }
                #[cfg(any(debug_assertions, feature = "sanitize"))]
                {
                    std::ptr::copy_nonoverlapping(sptr, temp[..].as_mut_ptr(), nb);
                    load_v(temp[..].as_ptr())
                }
            };
            v.write_to_slice_unaligned_unchecked(std::slice::from_raw_parts_mut(dptr, LANES));

            cfg_if::cfg_if! {
                if #[cfg(all(target_arch = "aarch64", target_feature = "sve2"))] {
                    let mask = escaped_mask_at(sptr).clear_high_bits(LANES - nb);
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
                } else {
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
        let fmt = |value: &str, dst: &mut [u8]| -> usize {
            let dst_ref = unsafe { std::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(dst) };
            format_string(value, dst_ref, true)
        };

        assert_eq!(fmt("", &mut dst), 2);
        assert_eq!(dst[..2], *b"\"\"");
        assert_eq!(fmt("\x00", &mut dst), 8);
        assert_eq!(dst[..8], *b"\"\\u0000\"");
        assert_eq!(fmt("test", &mut dst), 6);
        assert_eq!(dst[..6], *b"\"test\"");
        assert_eq!(fmt("test\"test", &mut dst), 12);
        assert_eq!(dst[..12], *b"\"test\\\"test\"");
        assert_eq!(fmt("\\testtest\"", &mut dst), 14);
        assert_eq!(dst[..14], *b"\"\\\\testtest\\\"\"");

        let long_str = "this is a long string that should be \\\"quoted and escaped multiple \
                        times to test the performance and correctness of the function.";
        assert_eq!(fmt(long_str, &mut dst), 129 + 4);
        assert_eq!(dst[..133], *b"\"this is a long string that should be \\\\\\\"quoted and escaped multiple times to test the performance and correctness of the function.\"");
    }
}
