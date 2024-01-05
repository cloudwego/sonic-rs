use std::ops::{BitAnd, BitOr, BitOrAssign};

use derive_more::From;

use crate::util::simd::{self, Mask, Simd};

impl_lanes!(Simd256u, 32);
impl_lanes!(Mask256, 32);

#[derive(Debug)]
pub enum Simd256i {
    #[cfg(target_arch = "x86_64")]
    Avx2(simd::avx2::Simd256i),
    Fallback(simd::v256::Simd256i),
}

impl Simd for Simd256i {
    const LANES: usize = 16;
    type Mask = Mask256;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Self::Avx2(Simd::loadu(ptr));
        }

        Self::Fallback(Simd::loadu(ptr))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Avx2(avx2) => avx2.storeu(ptr),
            Self::Fallback(fallback) => fallback.storeu(ptr),
        }
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(rhs), Self::Avx2(lhs)) => rhs.eq(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.eq(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Self::Avx2(Simd::splat(ch));
        }

        Self::Fallback(Simd::splat(ch))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(rhs), Self::Avx2(lhs)) => rhs.le(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.le(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(rhs), Self::Avx2(lhs)) => rhs.gt(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.gt(lhs).into(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum Simd256u {
    #[cfg(target_arch = "x86_64")]
    Avx2(simd::avx2::Simd256u),
    Fallback(simd::v256::Simd256u),
}

impl Simd for Simd256u {
    const LANES: usize = 16;
    type Mask = Mask256;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Self::Avx2(Simd::loadu(ptr));
        }

        Self::Fallback(Simd::loadu(ptr))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Avx2(avx2) => avx2.storeu(ptr),
            Self::Fallback(fallback) => fallback.storeu(ptr),
        }
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(rhs), Self::Avx2(lhs)) => rhs.eq(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.eq(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Self::Avx2(Simd::splat(ch));
        }

        Self::Fallback(Simd::splat(ch))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(rhs), Self::Avx2(lhs)) => rhs.le(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.le(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(rhs), Self::Avx2(lhs)) => rhs.gt(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.gt(lhs).into(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, From)]
pub enum Mask256 {
    #[cfg(target_arch = "x86_64")]
    Avx2(simd::avx2::Mask256),
    Fallback(simd::v256::Mask256),
}

impl Mask for Mask256 {
    type BitMap = u32;

    fn bitmask(self) -> Self::BitMap {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Avx2(avx2) => avx2.bitmask(),
            Self::Fallback(fallback) => fallback.bitmask(),
        }
    }

    fn splat(b: bool) -> Self {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Self::Avx2(Mask::splat(b));
        }

        Self::Fallback(Mask::splat(b))
    }
}

impl BitAnd for Mask256 {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(lhs), Self::Avx2(rhs)) => lhs.bitand(rhs).into(),
            (Self::Fallback(lhs), Self::Fallback(rhs)) => lhs.bitand(rhs).into(),
            _ => unreachable!(),
        }
    }
}

impl BitOr for Mask256 {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(lhs), Self::Avx2(rhs)) => lhs.bitor(rhs).into(),
            (Self::Fallback(lhs), Self::Fallback(rhs)) => lhs.bitor(rhs).into(),
            _ => unreachable!(),
        }
    }
}

impl BitOrAssign for Mask256 {
    fn bitor_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Avx2(lhs), Self::Avx2(rhs)) => lhs.bitor_assign(rhs),
            (Self::Fallback(lhs), Self::Fallback(rhs)) => lhs.bitor_assign(rhs),
            _ => unreachable!(),
        };
    }
}
