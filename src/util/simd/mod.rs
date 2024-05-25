pub mod bits;
mod traits;

// Link to all the SIMD backends by default for later runtime choice
cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub(crate) mod avx2;
        pub(crate) mod sse2;
    } else if #[cfg(target_arch = "aarch64")] {
        pub(crate) mod neon;
    }
}

pub(crate) mod v128;
pub(crate) mod v256;
pub(crate) mod v512;

#[doc(hidden)]
#[macro_export]
macro_rules! impl_lanes {
    ([$($impl_statement:tt)+] $lane: expr) => {
        $($impl_statement)+ {
            pub const fn lanes() -> usize {
                $lane
            }
        }
    };
}

// pick v128 simd
cfg_if::cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        use self::sse2::*;
    } else if #[cfg(all(target_feature="neon"))] {
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

pub use self::traits::{BitMask, Mask, Simd};
// pick v512 simd
// TODO: support avx512?
use self::v512::*;

pub type u8x16 = Simd128u;
pub type u8x32 = Simd256u<u8x16>;
pub type u8x64 = Simd512u<u8x32>;

pub type i8x32 = Simd256i<Simd128i>;

pub type m8x16 = Mask128;
pub type m8x32 = Mask256<m8x16>;
pub type m8x64 = Mask512<m8x32>;
