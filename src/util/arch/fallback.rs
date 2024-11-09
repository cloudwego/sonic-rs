// Not use PMULL instructions, but it is apparently slow.
// This is copied from simdjson.
pub unsafe fn prefix_xor(bitmask: u64) -> u64 {
    let mut bitmask = bitmask;
    bitmask ^= bitmask << 1;
    bitmask ^= bitmask << 2;
    bitmask ^= bitmask << 4;
    bitmask ^= bitmask << 8;
    bitmask ^= bitmask << 16;
    bitmask ^= bitmask << 32;
    bitmask
}

#[inline(always)]
pub unsafe fn get_nonspace_bits(data: &[u8; 64]) -> u64 {
    let mut mask: u64 = 0;
    for (i, p) in data.iter().enumerate() {
        if !matches!(*p, b'\t' | b'\n' | b'\r' | b' ') {
            mask |= 1 << i;
        }
    }
    mask
}
