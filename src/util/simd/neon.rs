use std::arch::aarch64::*;

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128u(uint8x16_t);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128i(int8x16_t);

impl Simd for Simd128i {
    const LANES: usize = 16;
    type Mask = Mask128;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        Self(vld1q_s8(ptr as *const i8))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        vst1q_s8(ptr as *mut i8, self.0);
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        Mask128(vceqq_s8(self.0, lhs.0))
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        Self(vdupq_n_s8(ch as i8))
    }

    // less or equal
    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        Mask128(vcleq_s8(self.0, lhs.0))
    }

    // greater than
    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        Mask128(vcgtq_s8(self.0, lhs.0))
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask128(uint8x16_t);

impl Mask for Mask128 {
    type BitMap = u16;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMap {
        vreinterpretq_u16_u8(self.0).into()
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        let v: i8 = if b { -1 } else { 0 };
        Self(vdupq_n_u8(v as u8))
    }
}

// Bitwise AND for Mask128
impl std::ops::BitAnd<Mask128> for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask128) -> Self::Output {
        Self(vandq_u8(self.0, rhs.0))
    }
}

// Bitwise OR for Mask128
impl std::ops::BitOr<Mask128> for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Mask128) -> Self::Output {
        Self(vorrq_u8(self.0, rhs.0))
    }
}

// Bitwise OR assignment for Mask128
impl std::ops::BitOrAssign<Mask128> for Mask128 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Mask128) {
        self.0 = vorrq_u8(self.0, rhs.0);
    }
}
