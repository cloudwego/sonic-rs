#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub fn prefix_xor(bitmask: u64) -> u64 {
    unsafe {
        let all_ones = _mm_set1_epi8(-1i8);
        let result = _mm_clmulepi64_si128(_mm_set_epi64x(0, bitmask as i64), all_ones, 0);
        _mm_cvtsi128_si64(result) as u64
    }
}

#[inline(always)]
pub fn get_nonspace_bits(data: &[u8; 64]) -> u64 {
    unsafe {
        let lo: std::arch::x86_64::__m256i = _mm256_loadu_si256(data.as_ptr() as *const __m256i);
        let hi: std::arch::x86_64::__m256i =
            _mm256_loadu_si256(data.as_ptr().offset(32) as *const __m256i);
        let whitespace_data = _mm256_setr_epi8(
            b' ' as i8,
            100,
            100,
            100,
            17,
            100,
            113,
            2,
            100,
            b'\t' as i8,
            b'\n' as i8,
            112,
            100,
            b'\r' as i8,
            100,
            100,
            b' ' as i8,
            100,
            100,
            100,
            17,
            100,
            113,
            2,
            100,
            b'\t' as i8,
            b'\n' as i8,
            112,
            100,
            b'\r' as i8,
            100,
            100,
        );
        let shuf_lo = _mm256_shuffle_epi8(whitespace_data, lo);
        let shuf_hi = _mm256_shuffle_epi8(whitespace_data, hi);
        let lo = _mm256_cmpeq_epi8(lo, shuf_lo);
        let hi = _mm256_cmpeq_epi8(hi, shuf_hi);
        let space = _mm256_movemask_epi8(lo) as u32 as u64
            | ((_mm256_movemask_epi8(hi) as u32 as u64) << 32);
        !space
    }
}

macro_rules! packadd_1 {
    ($v:ident) => {
        let delta = _mm_set1_epi64x(0x010A010A010A010A);
        $v = _mm_maddubs_epi16($v, delta);
    };
}

macro_rules! packadd_2 {
    ($v:ident) => {
        let delta = _mm_set1_epi64x(0x0001006400010064);
        $v = _mm_madd_epi16($v, delta);
    };
}

macro_rules! packadd_4 {
    ($v:ident) => {
        $v = _mm_packus_epi32($v, $v);
        let delta = _mm_set_epi16(0, 0, 0, 0, 1, 10000, 1, 10000);
        $v = _mm_madd_epi16($v, delta);
    };
}

// simd add for 5 ~ 8 digits
macro_rules! simd_add_5_8 {
    ($v:ident, $nd:literal) => {{
        $v = _mm_slli_si128($v, 16 - $nd);
        packadd_1!($v);
        packadd_2!($v);
        (_mm_extract_epi32($v, 2) as u64) * 10000 + (_mm_extract_epi32($v, 3) as u64)
    }};
}

// simd add for 9 ~ 15 digits
macro_rules! simd_add_9_15 {
    ($v:ident, $nd:literal) => {{
        $v = _mm_slli_si128($v, 16 - $nd);
        packadd_1!($v);
        packadd_2!($v);
        packadd_4!($v);
        (_mm_extract_epi32($v, 0) as u64) * 100000000 + (_mm_extract_epi32($v, 1) as u64)
    }};
}

macro_rules! simd_add_16 {
    ($v:ident) => {{
        packadd_1!($v);
        packadd_2!($v);
        packadd_4!($v);
        (_mm_extract_epi32($v, 0) as u64) * 100000000 + (_mm_extract_epi32($v, 1) as u64)
    }};
}

#[inline(always)]
pub unsafe fn simd_str2int(c: &[u8], need: usize) -> (u64, usize) {
    debug_assert!(need <= 16);
    let data = _mm_loadu_si128(c.as_ptr() as *const __m128i);
    let zero = _mm_setzero_si128();
    let nine = _mm_set1_epi8(9);
    let zero_c = _mm_set1_epi8(b'0' as i8);

    let mut data = _mm_sub_epi8(data, zero_c);
    let lt_zero = _mm_cmpgt_epi8(zero, data);
    let gt_nine = _mm_cmpgt_epi8(data, nine);

    let is_num_end = _mm_or_si128(lt_zero, gt_nine);
    let is_num_end_int = _mm_movemask_epi8(is_num_end);

    // get the real parsed count
    let mut count = need;
    if is_num_end_int != 0 {
        let digits = is_num_end_int.trailing_zeros() as usize;
        if digits < need {
            count = digits;
        }
    }

    let sum = match count {
        1 => _mm_extract_epi8(data, 0) as u64,
        2 => (_mm_extract_epi8(data, 0) * 10 + _mm_extract_epi8(data, 1)) as u64,
        3 => {
            // shift to clear the non-digit ascii in vector
            data = _mm_slli_si128(data, 16 - 3);
            packadd_1!(data);
            // add the highest two lanes
            (_mm_extract_epi16(data, 6) * 100 + _mm_extract_epi16(data, 7)) as u64
        }
        4 => {
            data = _mm_slli_si128(data, 16 - 4);
            packadd_1!(data);
            (_mm_extract_epi16(data, 6) * 100 + _mm_extract_epi16(data, 7)) as u64
        }
        5 => simd_add_5_8!(data, 5),
        6 => simd_add_5_8!(data, 6),
        7 => simd_add_5_8!(data, 7),
        8 => simd_add_5_8!(data, 8),
        9 => simd_add_9_15!(data, 9),
        10 => simd_add_9_15!(data, 10),
        11 => simd_add_9_15!(data, 11),
        12 => simd_add_9_15!(data, 12),
        13 => simd_add_9_15!(data, 13),
        14 => simd_add_9_15!(data, 14),
        15 => simd_add_9_15!(data, 15),
        16 => simd_add_16!(data),
        _ => unreachable!(),
    };
    (sum, count)
}
