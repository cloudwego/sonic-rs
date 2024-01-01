use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{Mask128, Simd128i, Simd128u};
use crate::{
    impl_lanes,
    util::simd::{Mask, Simd},
};

impl_lanes!(Simd256u, 32);

impl_lanes!(Mask256, 32);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256u((Simd128u, Simd128u));

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256i((Simd128i, Simd128i));

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask256((Mask128, Mask128));

impl Mask for Mask256 {
    type BitMap = u32;

    #[inline(always)]
    fn bitmask(self) -> u32 {
        let lo = self.0 .0.bitmask() as u32;
        let hi = self.0 .1.bitmask() as u32;
        lo | (hi << 16)
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
    fn eq(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&lhs.0 .0);
        let hi = self.0 .1.eq(&lhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Simd256u((Simd128u::splat(ch), Simd128u::splat(ch)))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&lhs.0 .0);
        let hi = self.0 .1.le(&lhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&lhs.0 .0);
        let hi = self.0 .1.gt(&lhs.0 .1);
        Mask256((lo, hi))
    }
}

impl Simd for Simd256i {
    const LANES: usize = 32;

    type Mask = Mask256;

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
    fn eq(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&lhs.0 .0);
        let hi = self.0 .1.eq(&lhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Simd256i((Simd128i::splat(ch), Simd128i::splat(ch)))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.le(&lhs.0 .0);
        let hi = self.0 .1.le(&lhs.0 .1);
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        let lo = self.0 .0.gt(&lhs.0 .0);
        let hi = self.0 .1.gt(&lhs.0 .1);
        Mask256((lo, hi))
    }
}
