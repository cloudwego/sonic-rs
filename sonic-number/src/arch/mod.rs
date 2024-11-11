cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq", target_feature = "avx2", target_feature = "sse2"))] {
        mod x86_64;
        pub use x86_64::*;
    } else if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
        mod aarch64;
        pub use aarch64::*;
    } else {
        mod fallback;
        pub use fallback::*;
    }
}
