pub mod bits;
mod traits;

#[doc(hidden)]
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

// pick v128 simd
cfg_if::cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        mod sse2;
        use self::sse2::*;
    } else if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
        pub(crate) mod neon;
        use self::neon::*;
    } else {
        // TODO: support wasm
        mod v128;
        use self::v128::*;
    }
}

// pick v256 simd
cfg_if::cfg_if! {
    if #[cfg(target_feature = "avx2")] {
        mod avx2;
        use self::avx2::*;
    } else {
        mod v256;
        use self::v256::*;
    }
}

pub use self::traits::{BitMask, Mask, Simd};
// pick v512 simd
// TODO: support avx512?
mod v512;
use self::v512::*;

pub type u8x16 = Simd128u;
pub type u8x32 = Simd256u;
pub type u8x64 = Simd512u;

pub type i8x32 = Simd256i;

pub type m8x16 = Mask128;
pub type m8x32 = Mask256;
pub type m8x64 = Mask512;
