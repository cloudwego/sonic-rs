use std::{
    arch::x86_64::*,
    mem::transmute,
    ops::{BitAnd, BitOr, BitOrAssign},
};

use crate::{
    impl_lanes,
    util::simd::{Mask, Simd},
};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256u(__m256i);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd256i(__m256i);

impl Simd for Simd256i {
    const LANES: usize = 32;
    type Mask = Mask256;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Self(_mm256_loadu_si256(ptr as *const __m256i)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { _mm256_storeu_si256(ptr as *mut __m256i, self.0) }
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        unsafe {
            let eq = _mm256_cmpeq_epi8(self.0, lhs.0);
            Mask256(eq)
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Self(_mm256_set1_epi8(ch as i8)) }
    }

    // less or equal
    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask256(_mm256_cmpgt_epi8(lhs.0, self.0)) }
    }

    // greater than
    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask256(_mm256_cmpgt_epi8(self.0, lhs.0)) }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask256(__m256i);

impl_lanes!(Simd256u, 32);

impl_lanes!(Mask256, 32);

impl Mask for Mask256 {
    type BitMap = u32;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMap {
        unsafe { transmute::<i32, u32>(_mm256_movemask_epi8(self.0)) }
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        let v: i8 = if b { -1 } else { 0 };
        unsafe { Mask256(_mm256_set1_epi8(v)) }
    }
}

impl BitAnd<Mask256> for Mask256 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask256) -> Self::Output {
        unsafe { Mask256(_mm256_and_si256(self.0, rhs.0)) }
    }
}

impl BitOr<Mask256> for Mask256 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Mask256) -> Self::Output {
        unsafe { Mask256(_mm256_or_si256(self.0, rhs.0)) }
    }
}

impl BitOrAssign<Mask256> for Mask256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Mask256) {
        unsafe { self.0 = _mm256_or_si256(self.0, rhs.0) }
    }
}

impl Simd for Simd256u {
    const LANES: usize = 32;
    type Mask = Mask256;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        unsafe { Simd256u(_mm256_loadu_si256(ptr as *const __m256i)) }
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        unsafe { _mm256_storeu_si256(ptr as *mut __m256i, self.0) }
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        unsafe {
            let eq = _mm256_cmpeq_epi8(self.0, lhs.0);
            Mask256(eq)
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Simd256u(_mm256_set1_epi8(ch as i8)) }
    }

    // less or equal
    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        unsafe {
            let max = _mm256_max_epu8(self.0, lhs.0);
            let eq = _mm256_cmpeq_epi8(max, lhs.0);
            Mask256(eq)
        }
    }

    // greater than
    #[inline(always)]
    fn gt(&self, _lhs: &Self) -> Self::Mask {
        todo!()
    }
}
