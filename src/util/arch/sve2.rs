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
/// SVE2 implementation: Returns the index of the first non-space char (0-15).
/// Returns 16 if all characters are spaces.
#[inline(always)]
pub unsafe fn get_nonspace_index(data: &[u8; 16]) -> usize {
    let mut idx: u64 = 16; // Default to 16 (Not Found)
                           // 0x09 (Tab), 0x0A (LF), 0x0D (CR), 0x20 (Space)
    let tokens: u32 = 0x090a0d20;

    core::arch::asm!(
        "ptrue  p0.b, vl16",
        "ld1b   {{z0.b}}, p0/z, [{ptr}]",
        "mov    z1.s, {t:w}",

        // 1. Identify non-space characters
        // NMATCH sets the Z flag if NO non-spaces are found (all whitespace)
        "nmatch p1.b, p0/z, z0.b, z1.b",

        // 2. Fast Path: Branch if NO non-space characters were found.
        // b.none checks the Z flag set by nmatch.
        // If Z=1 (all spaces), we skip the calculation and keep idx=16.
        "b.none 1f",

        // 3. Slow Path (Found something): Calculate the exact index
        "brkb   p2.b, p0/z, p1.b", // Mask bits *after* the first match
        "cntp   {idx}, p0, p2.b",  // Count leading matches

        "1:",
        ptr = in(reg) data.as_ptr(),
        t = in(reg) tokens,
        idx = inout(reg) idx,
        out("z0") _, out("z1") _,
        out("p0") _, out("p1") _, out("p2") _,
    );

    idx as usize
}
