use std::{
    arch::wasm32::*,
    ops::{BitAnd, BitOr, BitOrAssign},
};

use super::{Mask, Simd};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128i(v128);

impl Simd for Simd128i {
    const LANES: usize = 16;

    type Element = i8;

    type Mask = Mask128;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        Self(v128_load(ptr as _))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        v128_store(ptr as _, self.0);
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        Mask128(i8x16_eq(self.0, rhs.0))
    }

    #[inline(always)]
    fn splat(elem: Self::Element) -> Self {
        Self(i8x16_splat(elem))
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        Mask128(i8x16_gt(self.0, rhs.0))
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        Mask128(i8x16_le(self.0, rhs.0))
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128u(v128);

impl Simd for Simd128u {
    const LANES: usize = 16;

    type Element = u8;

    type Mask = Mask128;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        Self(v128_load(ptr as _))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        v128_store(ptr as _, self.0);
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        Mask128(i8x16_eq(self.0, rhs.0))
    }

    #[inline(always)]
    fn splat(elem: Self::Element) -> Self {
        Self(u8x16_splat(elem))
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        Mask128(u8x16_gt(self.0, rhs.0))
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        Mask128(u8x16_le(self.0, rhs.0))
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask128(v128);

impl Mask for Mask128 {
    type Element = u8;

    type BitMask = u16;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        i8x16_bitmask(self.0)
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        Self(i8x16_splat(if b { -1 } else { 0 }))
    }
}

impl BitAnd for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(v128_and(self.0, rhs.0))
    }
}

impl BitOr for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(v128_or(self.0, rhs.0))
    }
}

impl BitOrAssign for Mask128 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 = v128_or(self.0, rhs.0)
    }
}
