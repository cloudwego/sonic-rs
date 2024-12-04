//! Common utilities, for internal use only.

// The code is cloned from [rust-lang](https://github.com/rust-lang/rust) and modified necessary parts.

/// Helper methods to process immutable bytes.
pub(crate) trait ByteSlice {
    /// Read 8 bytes as a 64-bit integer in little-endian order.
    fn read_u64(&self) -> u64;

    /// Write a 64-bit integer as 8 bytes in little-endian order.
    fn write_u64(&mut self, value: u64);

    /// Iteratively parse and consume digits from bytes.
    /// Returns the same bytes with consumed digits being
    /// elided.
    fn parse_digits(&self, func: impl FnMut(u8)) -> &Self;
}

impl ByteSlice for [u8] {
    #[inline(always)] // inlining this is crucial to remove bound checks
    fn read_u64(&self) -> u64 {
        let mut tmp = [0; 8];
        tmp.copy_from_slice(&self[..8]);
        u64::from_le_bytes(tmp)
    }

    #[inline(always)] // inlining this is crucial to remove bound checks
    fn write_u64(&mut self, value: u64) {
        self[..8].copy_from_slice(&value.to_le_bytes())
    }

    #[inline]
    fn parse_digits(&self, mut func: impl FnMut(u8)) -> &Self {
        let mut s = self;

        // FIXME: Can't use s.split_first() here yet,
        // see https://github.com/rust-lang/rust/issues/109328
        while let [c, s_next @ ..] = s {
            let c = c.wrapping_sub(b'0');
            if c < 10 {
                func(c);
                s = s_next;
            } else {
                break;
            }
        }

        s
    }
}

/// Determine if 8 bytes are all decimal digits.
/// This does not care about the order in which the bytes were loaded.
pub(crate) fn is_8digits(v: u64) -> bool {
    let a = v.wrapping_add(0x4646_4646_4646_4646);
    let b = v.wrapping_sub(0x3030_3030_3030_3030);
    (a | b) & 0x8080_8080_8080_8080 == 0
}

/// A custom 64-bit floating point type, representing `f * 2^e`.
/// e is biased, so it be directly shifted into the exponent bits.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct BiasedFp {
    /// The significant digits.
    pub f: u64,
    /// The biased, binary exponent.
    pub e: i32,
}

impl BiasedFp {
    #[inline]
    pub const fn zero_pow2(e: i32) -> Self {
        Self { f: 0, e }
    }
}