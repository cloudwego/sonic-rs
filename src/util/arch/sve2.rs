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

/// SVE2 implementation of `get_nonspace_bits`.
/// But this won't get the full bitmap
#[inline(always)]
pub unsafe fn get_nonspace_bits(data: &[u8; 16]) -> u64 {
    let mut index: u64;
    // 0x09 (Tab), 0x0A (LF), 0x0D (CR), 0x20 (Space)
    let tokens: u32 = 0x090a0d20;

    core::arch::asm!(
        "ptrue  p0.b, vl16",
        "ld1b   {{z0.b}}, p0/z, [{ptr}]",
        // broadcast token set
        "mov    z1.s, {t:w}",

        // nmatch: find token does not match
        "nmatch p1.b, p0/z, z0.b, z1.b",

        // locate
        "brkb   p1.b, p0/z, p1.b",
        // count number of true bits
        "cntp   {idx}, p0, p1.b",

        ptr = in(reg) data.as_ptr(),
        t = in(reg) tokens,
        idx = out(reg) index,
        out("z0") _, out("z1") _,
        out("p0") _, out("p1") _,
    );

    if index < 16 {
        1u64 << index
    } else {
        0
    }
}
