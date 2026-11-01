#![allow(non_camel_case_types)]
#![no_std]

pub mod bits;
mod traits;

// pick v128 simd
cfg_if::cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        mod sse2;
        use self::sse2::*;
    } else if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
        pub mod neon;
        use self::neon::*;
    } else if #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))] {
        mod wasm128;
        use self::wasm128::*;
    } else {
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
cfg_if::cfg_if! {
    if #[cfg(all(target_feature = "avx512f", feature = "avx512"))] {
        mod avx512;
        use self::avx512::*;
    } else {
        mod v512;
        use self::v512::*;
    }
}

pub type u8x16 = Simd128u;
pub type u8x32 = Simd256u;
pub type u8x64 = Simd512u;

pub type i8x16 = Simd128i;
pub type i8x32 = Simd256i;
pub type i8x64 = Simd512i;

pub type m8x32 = Mask256;

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{u8x16, u8x32, u8x64, BitMask, Mask, Simd};

    /// Every backend has to hand out lane-ordered bitmasks: lane `i` of the vector is bit `i`
    /// of the mask, whatever the byte order of the target is. The parser and `format_string`
    /// turn `first_offset`/`trailing_zeros` straight into byte offsets.
    fn check_lane_order<V: Simd<Element = u8>>() {
        let mut buf = [0u8; 64];
        for i in 0..V::LANES {
            buf.iter_mut().for_each(|b| *b = 0);
            buf[i] = b'"';
            let v = unsafe { V::from_slice_unaligned_unchecked(&buf[..V::LANES]) };
            let mask = v.eq(&V::splat(b'"')).bitmask();
            assert!(!mask.all_zero(), "lane {} of {}", i, V::LANES);
            assert_eq!(mask.first_offset(), i, "lane {} of {}", i, V::LANES);
            // Dropping the lanes from `i` upwards clears the only set bit, keeping lane `i`
            // leaves it in place. (`clear_high_bits` takes fewer than LANES bits.)
            if i > 0 {
                assert!(mask.clear_high_bits(V::LANES - i).all_zero());
            }
            if i + 1 < V::LANES {
                assert_eq!(mask.clear_high_bits(V::LANES - i - 1).first_offset(), i);
            }
        }
    }

    #[test]
    fn bitmask_is_lane_ordered() {
        check_lane_order::<u8x16>();
        check_lane_order::<u8x32>();
        check_lane_order::<u8x64>();
    }

    #[test]
    fn before_follows_lane_order() {
        let mut buf = [0u8; 32];
        buf[3] = b'"';
        buf[7] = b'\\';
        let v = unsafe { u8x32::from_slice_unaligned_unchecked(&buf) };
        let quote = v.eq(&u8x32::splat(b'"')).bitmask();
        let bs = v.eq(&u8x32::splat(b'\\')).bitmask();
        assert!(quote.before(&bs));
        assert!(!bs.before(&quote));
    }
}
