use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{Mask256, Simd256i, Simd256u};
use crate::{
    impl_lanes,
    util::simd::{Mask, Simd},
};

impl_lanes!(Simd512u, 64);

impl_lanes!(Mask512, 64);

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
    type BitMap = u64;

    #[inline(always)]
    fn bitmask(self) -> u64 {
        let lo = self.0 .0.bitmask() as u64;
        let hi = self.0 .1.bitmask() as u64;
        lo | (hi << 32)
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
    fn eq(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&lhs.0 .0);
        let hi = self.0 .1.eq(&lhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Simd512u((Simd256u::splat(ch), Simd256u::splat(ch)))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&lhs.0 .0);
        let hi = self.0 .1.le(&lhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&lhs.0 .0);
        let hi = self.0 .1.gt(&lhs.0 .1);
        Mask512((lo, hi))
    }
}

impl Simd for Simd512i {
    const LANES: usize = 64;

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
    fn eq(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&lhs.0 .0);
        let hi = self.0 .1.eq(&lhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Simd512i((Simd256i::splat(ch), Simd256i::splat(ch)))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&lhs.0 .0);
        let hi = self.0 .1.le(&lhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&lhs.0 .0);
        let hi = self.0 .1.gt(&lhs.0 .1);
        Mask512((lo, hi))
    }
}
