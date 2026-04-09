/// SWAR (SIMD Within A Register) integer parsing.
///
/// Based on simdjson's `parse_eight_digits_unrolled` technique.
/// Uses pure u64 arithmetic to process 8 ASCII digits at a time.
/// Requires little-endian byte order — the u64 load must place the first
/// string byte in the least-significant byte position.

#[cfg(target_endian = "big")]
compile_error!("SWAR digit parsing requires little-endian byte order");

/// Check if 8 consecutive bytes are all ASCII digits ('0'-'9').
#[inline(always)]
pub fn is_eight_digits(src: &[u8]) -> bool {
    debug_assert!(src.len() >= 8);
    let val = unsafe { core::ptr::read_unaligned(src.as_ptr() as *const u64) };
    let a = val.wrapping_add(0x4646464646464646); // 0x80 - '9' - 1
    let b = val.wrapping_sub(0x3030303030303030); // - '0'
    (a | b) & 0x8080808080808080 == 0
}

/// Parse exactly 8 ASCII digits into a u32 using SWAR.
///
/// Combines digits in 3 steps:
///   [d0,d1,d2,d3,d4,d5,d6,d7]  (8 single digits)
///   -> [d0*10+d1, d2*10+d3, d4*10+d5, d6*10+d7]  (4 two-digit values)
///   -> [pair01*100+pair23, pair45*100+pair67]       (2 four-digit values)
///   -> group0*10000 + group1                        (1 eight-digit value)
///
/// # Safety
/// Caller must ensure all 8 bytes at `src` are valid ASCII digits.
#[inline(always)]
pub fn parse_eight_digits(src: &[u8]) -> u32 {
    debug_assert!(src.len() >= 8);
    let mut val = unsafe { core::ptr::read_unaligned(src.as_ptr() as *const u64) };
    val -= 0x3030303030303030;
    val = (val.wrapping_mul(10).wrapping_add(val >> 8)) & 0x00FF00FF00FF00FF;
    val = (val.wrapping_mul(100).wrapping_add(val >> 16)) & 0x0000FFFF0000FFFF;
    val = val.wrapping_mul(10000).wrapping_add(val >> 32);
    val as u32
}

/// Parse up to `need` ASCII digits using SWAR (8-digit batches) with scalar tail.
///
/// Returns `(parsed_value, digits_parsed)`.
#[inline(always)]
pub unsafe fn swar_str2int(c: &[u8], need: usize) -> (u64, usize) {
    let mut sum = 0u64;
    let mut i = 0;

    // First 8-digit batch
    if need >= 8 && c.len() >= 8 && is_eight_digits(c) {
        sum = parse_eight_digits(c) as u64;
        i = 8;

        // Second 8-digit batch
        if need >= 16 && c.len() >= 16 && is_eight_digits(&c[8..]) {
            sum = sum * 100_000_000 + parse_eight_digits(&c[8..]) as u64;
            i = 16;
        }
    }

    // Scalar tail for remaining digits
    while i < need && i < c.len() && c.get_unchecked(i).is_ascii_digit() {
        sum = sum * 10 + (*c.get_unchecked(i) - b'0') as u64;
        i += 1;
    }

    (sum, i)
}

/// Tolerant SWAR: parse 1-8 leading digits from a buffer that may contain non-digit bytes.
///
/// Returns `(value, digit_count)`. If the first byte is not a digit, returns `(0, 0)`.
///
/// # Safety
/// Caller must ensure `src` has at least 8 readable bytes (padding is fine).
#[inline(always)]
pub unsafe fn parse_digits_tolerant(src: &[u8]) -> (u64, usize) {
    debug_assert!(src.len() >= 8);
    let val = core::ptr::read_unaligned(src.as_ptr() as *const u64);

    let a = val.wrapping_add(0x4646464646464646);
    let b = val.wrapping_sub(0x3030303030303030);
    let non_digit = (a | b) & 0x8080808080808080;

    let ndigits = if non_digit == 0 {
        8
    } else {
        (non_digit.trailing_zeros() / 8) as usize
    };

    if ndigits == 0 {
        return (0, 0);
    }

    if ndigits == 8 {
        return (parse_eight_digits(src) as u64, 8);
    }

    // Mask out non-digit bytes, left-justify, then SWAR multiply chain
    let mut d = val.wrapping_sub(0x3030303030303030);
    d &= (1u64 << (ndigits * 8)) - 1;
    d <<= (8 - ndigits) * 8;
    d = (d.wrapping_mul(10).wrapping_add(d >> 8)) & 0x00FF00FF00FF00FF;
    d = (d.wrapping_mul(100).wrapping_add(d >> 16)) & 0x0000FFFF0000FFFF;
    d = d.wrapping_mul(10000).wrapping_add(d >> 32);
    (d as u32 as u64, ndigits)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn test_is_eight_digits() {
        assert!(is_eight_digits(b"12345678"));
        assert!(is_eight_digits(b"00000000"));
        assert!(is_eight_digits(b"99999999"));
        assert!(!is_eight_digits(b"1234567a"));
        assert!(!is_eight_digits(b"a1234567"));
        assert!(!is_eight_digits(b"1234 678"));
    }

    #[test]
    fn test_parse_eight_digits() {
        assert_eq!(parse_eight_digits(b"12345678"), 12345678);
        assert_eq!(parse_eight_digits(b"00000001"), 1);
        assert_eq!(parse_eight_digits(b"99999999"), 99999999);
        assert_eq!(parse_eight_digits(b"00000000"), 0);
        assert_eq!(parse_eight_digits(b"10000000"), 10000000);
    }

    #[test]
    fn test_swar_str2int() {
        unsafe {
            // Short numbers (scalar tail)
            assert_eq!(swar_str2int(b"1 ", 1), (1, 1));
            assert_eq!(swar_str2int(b"12 ", 2), (12, 2));
            assert_eq!(swar_str2int(b"123 ", 3), (123, 3));
            assert_eq!(swar_str2int(b"1234567 ", 7), (1234567, 7));

            // 8-digit SWAR
            assert_eq!(swar_str2int(b"12345678 ", 8), (12345678, 8));
            assert_eq!(swar_str2int(b"12345678 ", 16), (12345678, 8));

            // 9-15 digits (8 SWAR + scalar tail)
            assert_eq!(swar_str2int(b"123456789 ", 16), (123456789, 9));
            assert_eq!(
                swar_str2int(b"123456789012345 ", 16),
                (123456789012345, 15)
            );

            // 16 digits (two SWAR batches)
            assert_eq!(
                swar_str2int(b"1234567890123456 ", 16),
                (1234567890123456, 16)
            );

            // 19 digits (two SWAR + 3 scalar)
            assert_eq!(
                swar_str2int(b"1234567890123456789 ", 19),
                (1234567890123456789, 19)
            );

            // Non-digit stops parsing
            assert_eq!(swar_str2int(b"123abc ", 16), (123, 3));
        }
    }

    #[test]
    fn test_parse_digits_tolerant() {
        unsafe {
            // All 8 digits
            assert_eq!(parse_digits_tolerant(b"12345678"), (12345678, 8));
            assert_eq!(parse_digits_tolerant(b"00000000"), (0, 8));
            assert_eq!(parse_digits_tolerant(b"99999999"), (99999999, 8));

            // Boundary: 1 digit
            assert_eq!(parse_digits_tolerant(b"1......."), (1, 1));
            assert_eq!(parse_digits_tolerant(b"0......."), (0, 1));
            assert_eq!(parse_digits_tolerant(b"9......."), (9, 1));

            // 2 digits
            assert_eq!(parse_digits_tolerant(b"12......"), (12, 2));
            assert_eq!(parse_digits_tolerant(b"43......"), (43, 2));

            // 3 digits
            assert_eq!(parse_digits_tolerant(b"123....."), (123, 3));

            // 7 digits
            assert_eq!(parse_digits_tolerant(b"1234567."), (1234567, 7));
            assert_eq!(parse_digits_tolerant(b"4333333."), (4333333, 7));

            // 0 digits (non-digit first byte)
            assert_eq!(parse_digits_tolerant(b".1234567"), (0, 0));
            assert_eq!(parse_digits_tolerant(b" 1234567"), (0, 0));

            // Mixed: digits then non-digit mid-stream
            assert_eq!(parse_digits_tolerant(b"12345.78"), (12345, 5));
            assert_eq!(parse_digits_tolerant(b"1234e678"), (1234, 4));
            assert_eq!(parse_digits_tolerant(b"123456 8"), (123456, 6));
        }
    }
}
