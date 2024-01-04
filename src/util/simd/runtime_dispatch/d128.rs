use std::ops::{BitAnd, BitOr, BitOrAssign};

use derive_more::From;

use crate::util::simd::{self, Mask, Simd};

impl_lanes!(Simd128u, 16);
impl_lanes!(Mask128, 16);

#[derive(Debug)]
pub enum Simd128i {
    #[cfg(target_arch = "x86_64")]
    Sse2(simd::sse2::Simd128i),
    #[cfg(target_arch = "aarch64")]
    Neon(simd::neon::Simd128i),
    Fallback(simd::v128::Simd128i),
}

impl Simd for Simd128i {
    const LANES: usize = 16;
    type Mask = Mask128;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("sse2") {
            return Self::Sse2(Simd::loadu(ptr));
        }

        #[cfg(target_arch = "aarch64")]
        return Self::Neon(Simd::loadu(ptr));

        Self::Fallback(Simd::loadu(ptr))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Sse2(sse2) => sse2.storeu(ptr),
            #[cfg(target_arch = "aarch64")]
            Self::Neon(neon) => neon.storeu(ptr),
            Self::Fallback(fallback) => fallback.storeu(ptr),
        }
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(rhs), Self::Sse2(lhs)) => rhs.eq(lhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(rhs), Self::Neon(lhs)) => rhs.eq(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.eq(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("sse2") {
            return Self::Sse2(Simd::splat(ch));
        }

        #[cfg(target_arch = "aarch64")]
        return Self::Neon(Simd::splat(ch));

        Self::Fallback(Simd::splat(ch))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(rhs), Self::Sse2(lhs)) => rhs.le(lhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(rhs), Self::Neon(lhs)) => rhs.le(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.le(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(rhs), Self::Sse2(lhs)) => rhs.gt(lhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(rhs), Self::Neon(lhs)) => rhs.gt(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.gt(lhs).into(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum Simd128u {
    #[cfg(target_arch = "x86_64")]
    Sse2(simd::sse2::Simd128u),
    #[cfg(target_arch = "aarch64")]
    Neon(simd::neon::Simd128u),
    Fallback(simd::v128::Simd128u),
}

impl Simd for Simd128u {
    const LANES: usize = 16;
    type Mask = Mask128;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("sse2") {
            return Self::Sse2(Simd::loadu(ptr));
        }

        #[cfg(target_arch = "aarch64")]
        return Self::Neon(Simd::loadu(ptr));

        Self::Fallback(Simd::loadu(ptr))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Sse2(sse2) => sse2.storeu(ptr),
            #[cfg(target_arch = "aarch64")]
            Self::Neon(neon) => neon.storeu(ptr),
            Self::Fallback(fallback) => fallback.storeu(ptr),
        }
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(rhs), Self::Sse2(lhs)) => rhs.eq(lhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(rhs), Self::Neon(lhs)) => rhs.eq(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.eq(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("sse2") {
            return Self::Sse2(Simd::splat(ch));
        }

        #[cfg(target_arch = "aarch64")]
        return Self::Neon(Simd::splat(ch));

        Self::Fallback(Simd::splat(ch))
    }

    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(rhs), Self::Sse2(lhs)) => rhs.le(lhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(rhs), Self::Neon(lhs)) => rhs.le(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.le(lhs).into(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        match (self, lhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(rhs), Self::Sse2(lhs)) => rhs.gt(lhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(rhs), Self::Neon(lhs)) => rhs.gt(lhs).into(),
            (Self::Fallback(rhs), Self::Fallback(lhs)) => rhs.gt(lhs).into(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, From)]
pub enum Mask128 {
    #[cfg(target_arch = "x86_64")]
    Sse2(simd::sse2::Mask128),
    #[cfg(target_arch = "aarch64")]
    Neon(simd::neon::Mask128),
    Fallback(simd::v128::Mask128),
}

impl Mask for Mask128 {
    type BitMap = u16;

    fn bitmask(self) -> Self::BitMap {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Sse2(sse2) => sse2.bitmask(),
            #[cfg(target_arch = "aarch64")]
            Self::Neon(neon) => neon.bitmask(),
            Self::Fallback(fallback) => fallback.bitmask(),
        }
    }

    fn splat(b: bool) -> Self {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("sse2") {
            return Self::Sse2(Mask::splat(b));
        }

        #[cfg(target_arch = "aarch64")]
        return Self::Neon(Mask::splat(b));

        Self::Fallback(Mask::splat(b))
    }
}

impl BitAnd for Mask128 {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(lhs), Self::Sse2(rhs)) => lhs.bitand(rhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(lhs), Self::Sse(rhs)) => lhs.bitand(rhs).into(),
            (Self::Fallback(lhs), Self::Fallback(rhs)) => lhs.bitand(rhs).into(),
            _ => unreachable!(),
        }
    }
}

impl BitOr for Mask128 {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(lhs), Self::Sse2(rhs)) => lhs.bitor(rhs).into(),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(lhs), Self::Sse(rhs)) => lhs.bitor(rhs).into(),
            (Self::Fallback(lhs), Self::Fallback(rhs)) => lhs.bitor(rhs).into(),
            _ => unreachable!(),
        }
    }
}

impl BitOrAssign for Mask128 {
    fn bitor_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            #[cfg(target_arch = "x86_64")]
            (Self::Sse2(lhs), Self::Sse2(rhs)) => lhs.bitor_assign(rhs),
            #[cfg(target_arch = "aarch64")]
            (Self::Neon(lhs), Self::Sse(rhs)) => lhs.bitor_assign(rhs),
            (Self::Fallback(lhs), Self::Fallback(rhs)) => lhs.bitor_assign(rhs),
            _ => unreachable!(),
        };
    }
}
