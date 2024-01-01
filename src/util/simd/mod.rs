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

// pick v128 simd
cfg_if::cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        mod sse2;
        use self::sse2::*;
    } else if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
        mod neon;
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

pub use self::traits::{Mask, Simd};
// pick v512 simd
// TODO: support avx512?
mod v512;
use self::v512::*;

pub type u8x16 = Simd128i;
pub type u8x32 = Simd256u;
pub type u8x64 = Simd512u;

pub type i8x32 = Simd256i;

pub type m8x16 = Mask128;
pub type m8x32 = Mask256;
pub type m8x64 = Mask512;

// #[inline(always)]
// pub fn simd_next_char<const L: usize, const N: usize>(
//     chunck: &[u8; L],
//     tokens: [u8; N],
// ) -> Option<(usize, u8)> { debug_assert!(L == 32 || L == 64); unsafe { let mut chunk_ptr =
//   chunck.as_ptr(); for i in 0..L / 32 { let v = _mm256_loadu_si256(chunk_ptr as *const __m256i);
//   let mut vor = _mm256_setzero_si256();

//             for token in tokens {
//                 let t = _mm256_set1_epi8(mem::transmute::<u8, i8>(token));
//                 vor = _mm256_or_si256(vor, _mm256_cmpeq_epi8(v, t));
//             }

//             let next = _mm256_movemask_epi8(vor);
//             if next != 0 {
//                 let cnt = next.trailing_zeros() as usize;
//                 let ch = chunck[cnt];
//                 return Some((cnt, ch));
//             }
//             chunk_ptr = unsafe { chunk_ptr.add(32) };
//         }
//     }
//     None
// }

// #[inline(always)]
// pub fn simd_bitmask_32(chunck: &[u8; 32], ch: u8) -> u32 {
//     unsafe {
//         let mut chunk_ptr = chunck.as_ptr();
//         let v = _mm256_loadu_si256(chunk_ptr as *const __m256i);
//         let t = _mm256_set1_epi8(mem::transmute::<u8, i8>(ch));
//         let t = _mm256_cmpeq_epi8(v, t);
//         let mask = _mm256_movemask_epi8(t);
//         transmute::<i32, u32>(mask)
//     }
// }

// #[inline(always)]
// pub fn simd_bitmask_64(chunck: &[u8; 64], ch: u8) -> u64 {
//     unsafe {
//         let chunck1 = transmute(chunck);
//         let mask1 = simd_bitmask_32(chunck1, ch) as u64;

//         let chunck2 = &*(chunck.as_ptr().add(32) as *const [u8; 32]);
//         let mask2 = simd_bitmask_32(chunck2, ch) as u64;
//         mask1 | (mask2 << 32)
//     }
// }
