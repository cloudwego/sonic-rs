#[inline(always)]
pub unsafe fn simd_str2int(c: &[u8], need: usize) -> (u64, usize) {
    debug_assert!(need < 17);
    let mut sum = 0u64;
    let mut i = 0;
    while i < need && c.get_unchecked(i).is_ascii_digit() {
        sum = (c.get_unchecked(i) - b'0') as u64 + sum * 10;
        i += 1;
    }
    (sum, i)
}
