use std::arch::x86_64::*;

#[inline(always)]
pub unsafe fn prefix_xor(bitmask: u64) -> u64 {
    unsafe {
        let all_ones = _mm_set1_epi8(-1i8);
        let result = _mm_clmulepi64_si128(_mm_set_epi64x(0, bitmask as i64), all_ones, 0);
        _mm_cvtsi128_si64(result) as u64
    }
}

#[inline(always)]
pub unsafe fn get_nonspace_bits(data: &[u8; 64]) -> u64 {
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
