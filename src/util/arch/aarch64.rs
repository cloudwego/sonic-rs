// Not use PMULL instructions, but it is apparently slow.
// This is copied from simdjson.
pub fn prefix_xor(bitmask: u64) -> u64 {
    let mut bitmask = bitmask;
    bitmask ^= bitmask << 1;
    bitmask ^= bitmask << 2;
    bitmask ^= bitmask << 4;
    bitmask ^= bitmask << 8;
    bitmask ^= bitmask << 16;
    bitmask ^= bitmask << 32;
    return bitmask;
}

#[inline(always)]
pub fn get_nonspace_bits(data: &[u8; 64]) -> u64 {
    let mut mask: u64 = 0;
    for (i, p) in data.iter().enumerate() {
        if !matches!(*p, b'\t' | b'\n' | b'\r' | b' ') {
            mask |= 1 << i;
        }
    }
    mask
}

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
