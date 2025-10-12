use std::{
    arch::x86_64::*,
    ops::{BitAnd, BitOr, BitOrAssign},
};

use super::{Mask, Simd};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd512u(__m512i);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd512i(__m512i);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Mask512(__mmask64);

impl Mask for Mask512 {
    type BitMask = u64;
    type Element = u8;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        self.0
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        if b {
            Mask512(u64::MAX)
        } else {
            Mask512(0)
        }
    }
}

impl BitOr for Mask512 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Mask512(self.0 | rhs.0)
    }
}

impl BitOrAssign for Mask512 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd<Mask512> for Mask512 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask512) -> Self::Output {
        Mask512(self.0 & rhs.0)
    }
}

impl Simd for Simd512u {
    const LANES: usize = 64;
    type Element = u8;
    type Mask = Mask512;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Simd512u(_mm512_loadu_si512(ptr as *const __m512i)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { _mm512_storeu_si512(ptr as *mut __m512i, self.0) }
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmpeq_epi8_mask(self.0, rhs.0)) }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Simd512u(_mm512_set1_epi8(ch as i8)) }
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmple_epu8_mask(self.0, rhs.0)) }
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmpgt_epu8_mask(self.0, rhs.0)) }
    }
}

impl Simd for Simd512i {
    const LANES: usize = 64;
    type Element = i8;
    type Mask = Mask512;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Simd512i(_mm512_loadu_si512(ptr as *const __m512i)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { _mm512_storeu_si512(ptr as *mut __m512i, self.0) }
    }

    #[inline(always)]
    fn eq(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmpeq_epi8_mask(self.0, rhs.0)) }
    }

    #[inline(always)]
    fn splat(elem: i8) -> Self {
        unsafe { Simd512i(_mm512_set1_epi8(elem)) }
    }

    #[inline(always)]
    fn le(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmple_epi8_mask(self.0, rhs.0)) }
    }

    #[inline(always)]
    fn gt(&self, rhs: &Self) -> Self::Mask {
        unsafe { Mask512(_mm512_cmpgt_epi8_mask(self.0, rhs.0)) }
    }
}
