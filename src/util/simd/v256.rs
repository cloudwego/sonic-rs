use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{bits::combine_u16, BitMask, Mask, Simd};
use crate::impl_lanes;

impl_lanes!([impl<B: Simd> Simd256u<B>] 32);

impl_lanes!([impl<M: Mask> Mask256<M>] 32);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256u<B: Simd = super::Simd128u>((B, B));

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256i<B: Simd = super::Simd128i>((B, B));

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask256<M: Mask = super::Mask128>(pub(crate) (M, M));

impl<M: Mask> Mask for Mask256<M>
where
    <M as Mask>::BitMask: BitMask<Primitive = u16>,
{
    type BitMask = u32;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        #[cfg(target_arch = "aarch64")]
        if super::neon::is_supported() {
            let (v0, v1) = self.0;
            return unsafe { super::neon::to_bitmask32(v0.0, v1.0) };
        }

        combine_u16(
            self.0 .0.bitmask().as_primitive(),
            self.0 .1.bitmask().as_primitive(),
        )
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        Mask256((M::splat(b), M::splat(b)))
    }
}

impl<M: Mask> BitOr for Mask256<M> {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        let lo = self.0 .0 | rhs.0 .0;
        let hi = self.0 .1 | rhs.0 .1;
        Mask256((lo, hi))
    }
}

impl<M: Mask> BitOrAssign for Mask256<M> {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 .0 |= rhs.0 .0;
        self.0 .1 |= rhs.0 .1;
    }
}

impl<M: Mask> BitAnd<Mask256<M>> for Mask256<M> {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask256<M>) -> Self::Output {
        let lo = self.0 .0 & rhs.0 .0;
        let hi = self.0 .1 & rhs.0 .1;
        Mask256((lo, hi))
    }
}

impl<B> Simd for Simd256u<B>
where
    B: Simd<Element = u8>,
    <B::Mask as Mask>::BitMask: BitMask<Primitive = u16>,
{
    const LANES: usize = 32;

    type Mask = Mask256<B::Mask>;
    type Element = u8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = B::loadu(ptr);
        let hi = B::loadu(ptr.add(B::LANES));
        Simd256u((lo, hi))
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
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn splat(elem: u8) -> Self {
        Simd256u((B::splat(elem), B::splat(elem)))
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

impl<B> Simd for Simd256i<B>
where
    B: Simd<Element = i8>,
    <B::Mask as Mask>::BitMask: BitMask<Primitive = u16>,
{
    const LANES: usize = 32;

    type Mask = Mask256<B::Mask>;
    type Element = i8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        let lo = B::loadu(ptr);
        let hi = B::loadu(ptr.add(B::LANES));
        Simd256i((lo, hi))
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
        Mask256((lo, hi))
    }

    #[inline(always)]
    fn splat(elem: i8) -> Self {
        Simd256i((B::splat(elem), B::splat(elem)))
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
