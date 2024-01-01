cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use x86_64::*;
    } else {
        // TODO: support aarch64 native
        mod fallback;
        pub use fallback::*;
    }
}

#[inline]
pub fn page_size() -> usize {
    cfg_if::cfg_if! {
        // fast path for most common arch
        if #[cfg(any(target_os = "linux", target_os = "macos"))] {
            4096
        } else {
            // slow path for portability
            ::page_size::get()
        }
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
        assert_eq!(
            non_space_bits, expected_bits,
            "bits is {:b}",
            non_space_bits
        );
    }
}
