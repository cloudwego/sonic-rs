use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{Mask, Mask128, Simd, Simd128i, Simd128u};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256u((Simd128u, Simd128u));

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256i((Simd128i, Simd128i));

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask256(pub(crate) (Mask128, Mask128));

impl Mask for Mask256 {
    type BitMask = u32;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        cfg_if::cfg_if! {
            if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
                let(v0, v1) = self.0;
                unsafe { super::neon::to_bitmask32(v0.0, v1.0) }
            } else {
                fn combine_u16(lo: u16, hi: u16) -> u32 {
                    #[cfg(target_endian = "little")]
                    {
                        (lo as u32) | ((hi as u32) << 16)
                    }
                    #[cfg(target_endian = "big")]
                    {
                        (hi as u32) | ((lo as u32) << 16)
                    }
                }
                combine_u16(self.0 .0.bitmask(), self.0 .1.bitmask())
            }
        }
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        Mask256((Mask128::splat(b), Mask128::splat(b)))
    }
}

impl BitOr for Mask256 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        let lo = self.0 .0 | rhs.0 .0;
        let hi = self.0 .1 | rhs.0 .1;
        Mask256((lo, hi))
    }
}

impl BitOrAssign for Mask256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 .0 |= rhs.0 .0;
        self.0 .1 |= rhs.0 .1;
    }
}

impl BitAnd<Mask256> for Mask256 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask256) -> Self::Output {
        let lo = self.0 .0 & rhs.0 .0;
        let hi = self.0 .1 & rhs.0 .1;
        Mask256((lo, hi))
    }
}

impl Simd for Simd256u {
    const LANES: usize = 32;

    type Mask = Mask256;
    type Element = u8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = Simd128u::loadu(ptr);
        let hi = Simd128u::loadu(ptr.add(Simd128u::LANES));
        Simd256u((lo, hi))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        Simd128u::storeu(&self.0 .0, ptr);
        Simd128u::storeu(&self.0 .1, ptr.add(Simd128u::LANES));
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&rhs.0 .0);
        let hi = self.0 .1.eq(&rhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn splat(elem: u8) -> Self {
        Simd256u((Simd128u::splat(elem), Simd128u::splat(elem)))
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&rhs.0 .0);
        let hi = self.0 .1.le(&rhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&rhs.0 .0);
        let hi = self.0 .1.gt(&rhs.0 .1);
        Mask256((lo, hi))
    }
}

impl Simd for Simd256i {
    const LANES: usize = 32;

    type Mask = Mask256;
    type Element = i8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = Simd128i::loadu(ptr);
        let hi = Simd128i::loadu(ptr.add(Simd128i::LANES));
        Simd256i((lo, hi))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        Simd128i::storeu(&self.0 .0, ptr);
        Simd128i::storeu(&self.0 .1, ptr.add(Simd128i::LANES));
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&rhs.0 .0);
        let hi = self.0 .1.eq(&rhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn splat(elem: i8) -> Self {
        Simd256i((Simd128i::splat(elem), Simd128i::splat(elem)))
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&rhs.0 .0);
        let hi = self.0 .1.le(&rhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&rhs.0 .0);
        let hi = self.0 .1.gt(&rhs.0 .1);
        Mask256((lo, hi))
    }
}
