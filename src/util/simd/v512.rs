use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{bits::combine_u32, BitMask, Mask, Simd};
use crate::impl_lanes;

impl_lanes!([impl<B: Simd> Simd512u<B>] 64);

impl_lanes!([impl<M: Mask> Mask512<M>] 64);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd512u<B: Simd = super::Simd256u>((B, B));

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd512i<B: Simd = super::Simd256i>((B, B));

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask512<M: Mask = super::Mask256>((M, M));

impl<M: Mask> Mask for Mask512<M>
where
    <M as Mask>::BitMask: BitMask<Primitive = u32>,
{
    type BitMask = u64;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        #[cfg(target_arch = "aarch64")]
        if super::neon::is_supported() {
            let (v0, v1) = self.0;
            let (m0, m1) = v0.0;
            let (m2, m3) = v1.0;
            return unsafe { super::neon::to_bitmask64(m0.0, m1.0, m2.0, m3.0) };
        }

        combine_u32(
            self.0 .0.bitmask().as_primitive(),
            self.0 .1.bitmask().as_primitive(),
        )
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        Mask512((M::splat(b), M::splat(b)))
    }
}

impl<M: Mask> BitOr for Mask512<M> {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        let lo = self.0 .0 | rhs.0 .0;
        let hi = self.0 .1 | rhs.0 .1;
        Mask512((lo, hi))
    }
}

impl<M: Mask> BitOrAssign for Mask512<M> {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 .0 |= rhs.0 .0;
        self.0 .1 |= rhs.0 .1;
    }
}

impl<M: Mask> BitAnd<Mask512<M>> for Mask512<M> {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask512<M>) -> Self::Output {
        let lo = self.0 .0 & rhs.0 .0;
        let hi = self.0 .1 & rhs.0 .1;
        Mask512((lo, hi))
    }
}

impl<B> Simd for Simd512u<B>
where
    B: Simd<Element = u8>,
    <B::Mask as Mask>::BitMask: BitMask<Primitive = u32>,
{
    const LANES: usize = 64;
    type Element = u8;
    type Mask = Mask512<B::Mask>;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = B::loadu(ptr);
        let hi = B::loadu(ptr.add(B::LANES));
        Simd512u((lo, hi))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        B::storeu(&self.0 .0, ptr);
        B::storeu(&self.0 .1, ptr.add(B::LANES));
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&rhs.0 .0);
        let hi = self.0 .1.eq(&rhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Simd512u((B::splat(ch), B::splat(ch)))
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

impl<B> Simd for Simd512i<B>
where
    B: Simd<Element = i8>,
    <B::Mask as Mask>::BitMask: BitMask<Primitive = u32>,
{
    const LANES: usize = 64;
    type Element = i8;

    type Mask = Mask512<B::Mask>;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = B::loadu(ptr);
        let hi = B::loadu(ptr.add(B::LANES));
        Simd512i((lo, hi))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        B::storeu(&self.0 .0, ptr);
        B::storeu(&self.0 .1, ptr.add(B::LANES));
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        let lo = self.0 .0.eq(&rhs.0 .0);
        let hi = self.0 .1.eq(&rhs.0 .1);
        Mask512((lo, hi))
    }

    #[inline(always)]
    fn splat(elem: i8) -> Self {
        Simd512i((B::splat(elem), B::splat(elem)))
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
