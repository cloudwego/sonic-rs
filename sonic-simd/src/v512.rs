use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{Mask, Mask256, Simd, Simd256i, Simd256u};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd512u((Simd256u, Simd256u));

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd512i((Simd256i, Simd256i));

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask512((Mask256, Mask256));

impl Mask for Mask512 {
    type BitMask = u64;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        cfg_if::cfg_if! {
            if #[cfg(all(target_feature="neon", target_arch="aarch64"))] {
                let (v0, v1) = self.0;
                let (m0, m1) = v0.0;
                let (m2, m3) = v1.0;
                unsafe { super::neon::to_bitmask64(m0.0, m1.0, m2.0, m3.0) }
            } else {
                fn combine_u32(lo: u32, hi: u32) -> u64 {
                    #[cfg(target_endian = "little")]
                    {
                        (lo as u64) | ((hi as u64) << 32)
                    }
                    #[cfg(target_endian = "big")]
                    {
                        (hi as u64) | ((lo as u64) << 32)
                    }
                }
                combine_u32(self.0 .0.bitmask(), self.0 .1.bitmask())
            }
        }
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        Mask512((Mask256::splat(b), Mask256::splat(b)))
    }
}

impl BitOr for Mask512 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        let lo = self.0 .0 | rhs.0 .0;
        let hi = self.0 .1 | rhs.0 .1;
        Mask512((lo, hi))
    }
}

impl BitOrAssign for Mask512 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 .0 |= rhs.0 .0;
        self.0 .1 |= rhs.0 .1;
    }
}

impl BitAnd<Mask512> for Mask512 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask512) -> Self::Output {
        let lo = self.0 .0 & rhs.0 .0;
        let hi = self.0 .1 & rhs.0 .1;
        Mask512((lo, hi))
    }
}

impl Simd for Simd512u {
    const LANES: usize = 64;
    type Element = u8;
    type Mask = Mask512;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = Simd256u::loadu(ptr);
        let hi = Simd256u::loadu(ptr.add(Simd256u::LANES));
        Simd512u((lo, hi))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        Simd256u::storeu(&self.0 .0, ptr);
        Simd256u::storeu(&self.0 .1, ptr.add(Simd256u::LANES));
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&rhs.0 .0);
        let hi = self.0 .1.eq(&rhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Simd512u((Simd256u::splat(ch), Simd256u::splat(ch)))
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&rhs.0 .0);
        let hi = self.0 .1.le(&rhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&rhs.0 .0);
        let hi = self.0 .1.gt(&rhs.0 .1);
        Mask512((lo, hi))
    }
}

impl Simd for Simd512i {
    const LANES: usize = 64;
    type Element = i8;

    type Mask = Mask512;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = Simd256i::loadu(ptr);
        let hi = Simd256i::loadu(ptr.add(Simd256i::LANES));
        Simd512i((lo, hi))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        Simd256i::storeu(&self.0 .0, ptr);
        Simd256i::storeu(&self.0 .1, ptr.add(Simd256i::LANES));
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&rhs.0 .0);
        let hi = self.0 .1.eq(&rhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn splat(elem: i8) -> Self {
        Simd512i((Simd256i::splat(elem), Simd256i::splat(elem)))
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&rhs.0 .0);
        let hi = self.0 .1.le(&rhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&rhs.0 .0);
        let hi = self.0 .1.gt(&rhs.0 .1);
        Mask512((lo, hi))
    }
}
