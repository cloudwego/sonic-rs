cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use x86_64::*;
    } else if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use aarch64::*;
    } else {
        panic!("not support architecture");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_non_space_bits() {
        let input = b"\t\r\n xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let non_space_bits = get_nonspace_bits(input);
        let expected_bits = 0b1111111111111111111111111111111111111111111111111111111111110000;
        assert_eq!(
            non_space_bits, expected_bits,
            "bits is {:b}",
            non_space_bits
        );
    }
}
