mod traits;

#[macro_export]
macro_rules! impl_lanes {
    ($simd: ty, $lane: expr) => {
        impl $simd {
            pub const fn lanes() -> usize {
                $lane
            }
        }
    };
}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod avx2;
        mod sse2;
    } else if #[cfg(target_arch = "aarch64")] {
        pub(crate) mod neon;
    }
}

mod v128;
mod v256;
mod v512;

pub use self::traits::{Mask, Simd};

cfg_if::cfg_if! {
    if #[cfg(feature = "runtime-detection")] {
        mod runtime_dispatch;
        pub use self::runtime_dispatch::*;
    } else {
        // pick v128 simd
        cfg_if::cfg_if! {
            if #[cfg(target_feature = "sse2")] {
                use self::sse2::*;
            } else if #[cfg(target_arch="aarch64")] {
                use self::neon::*;
            } else {
                // TODO: support wasm
                use self::v128::*;
            }
        }

        // pick v256 simd
        cfg_if::cfg_if! {
            if #[cfg(target_feature = "avx2")] {
                use self::avx2::*;
            } else {
                use self::v256::*;
            }
        }

        // pick v512 simd
        // TODO: support avx512?
        use self::v512::*;
    }
}

pub type u8x16 = Simd128i;
pub type u8x32 = Simd256u;
pub type u8x64 = Simd512u;

pub type i8x32 = Simd256i;

pub type m8x16 = Mask128;
pub type m8x32 = Mask256;
pub type m8x64 = Mask512;
