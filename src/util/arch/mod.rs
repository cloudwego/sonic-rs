cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq", target_feature = "avx2", target_feature = "sse2"))] {
        mod x86_64;
        pub use x86_64::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        mod fallback;
        mod runtime;
        pub use runtime::*;
    } else if #[cfg(all(target_feature = "neon", target_arch = "aarch64"))] {
        mod aarch64;
        pub use aarch64::*;
    } else {
        mod fallback;
        pub use fallback::*;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_non_space_bits() {
        let input = b"\t\r\n xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let non_space_bits = unsafe { get_nonspace_bits(input) };
        let expected_bits = 0b1111111111111111111111111111111111111111111111111111111111110000;
        assert_eq!(non_space_bits, expected_bits, "bits is {non_space_bits:b}");
    }

    #[test]
    fn test_prefix_xor() {
        // prefix_xor computes a running XOR: each bit in the output is the XOR of itself
        // and all lower bits in the input. For input 0b0100, the result is 0b1100...0 (all
        // bits from position 2 upward are flipped).
        let result = unsafe { prefix_xor(0b0100) };
        assert_eq!(result, 0xFFFF_FFFF_FFFF_FFFC);

        // Two set bits cancel each other in the prefix XOR — the region between them is 1,
        // outside is 0.
        let result = unsafe { prefix_xor(0b1010) };
        assert_eq!(result, 0x0000_0000_0000_0006);

        // Single bit at position 0.
        let result = unsafe { prefix_xor(1) };
        assert_eq!(result, u64::MAX);

        // Zero input → zero output.
        let result = unsafe { prefix_xor(0) };
        assert_eq!(result, 0);
    }
}
