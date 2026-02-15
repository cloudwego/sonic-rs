cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq", target_feature = "avx2", target_feature = "sse2"))] {
        mod x86_64;
        pub use x86_64::*;
    } else if #[cfg(all(target_feature="sve2", target_arch="aarch64"))] {
        mod sve2;
        pub use sve2::*;
    } else if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
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
        cfg_if::cfg_if! {
            if #[cfg(all(target_feature="sve2", target_arch="aarch64"))] {
                let non_space_bits = unsafe { get_nonspace_bits(std::mem::transmute(input)) };
                // sve2 cannot generate the full bitmap(without performance loss)
                let expected_bits = 0b10000;
                assert_eq!(non_space_bits, expected_bits, "bits is {non_space_bits:b}");
            } else {
                let non_space_bits = unsafe { get_nonspace_bits(input) };
                let expected_bits = 0b1111111111111111111111111111111111111111111111111111111111110000;
                assert_eq!(non_space_bits, expected_bits, "bits is {non_space_bits:b}");
            }
        }
    }
}
