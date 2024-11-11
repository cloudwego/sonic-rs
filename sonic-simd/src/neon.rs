use std::arch::aarch64::*;

use super::{bits::NeonBits, Mask, Simd};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128u(uint8x16_t);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128i(int8x16_t);

impl Simd for Simd128u {
    const LANES: usize = 16;
    type Mask = Mask128;
    type Element = u8;

    #[inline(always)]
    unsafe fn loadu(ptr: *const u8) -> Self {
        Self(vld1q_u8(ptr))
    }

    #[inline(always)]
    unsafe fn storeu(&self, ptr: *mut u8) {
        vst1q_u8(ptr, self.0);
    }

    #[inline(always)]
    fn eq(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vceqq_u8(self.0, lhs.0)) }
    }

    #[inline(always)]
    fn splat(ch: u8) -> Self {
        unsafe { Self(vdupq_n_u8(ch)) }
    }

    // less or equal
    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vcleq_u8(self.0, lhs.0)) }
    }

    // greater than
    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vcgtq_u8(self.0, lhs.0)) }
    }
}

impl Simd for Simd128i {
    const LANES: usize = 16;
    type Mask = Mask128;
    type Element = i8;

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
        unsafe { Mask128(vceqq_s8(self.0, lhs.0)) }
    }

    #[inline(always)]
    fn splat(elem: i8) -> Self {
        unsafe { Self(vdupq_n_s8(elem)) }
    }

    // less or equal
    #[inline(always)]
    fn le(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vcleq_s8(self.0, lhs.0)) }
    }

    // greater than
    #[inline(always)]
    fn gt(&self, lhs: &Self) -> Self::Mask {
        unsafe { Mask128(vcgtq_s8(self.0, lhs.0)) }
    }
}

pub(crate) const BIT_MASK_TAB: [u8; 16] = [
    0x01u8, 0x02, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80, 0x01, 0x02, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80,
];

#[derive(Debug)]
#[repr(transparent)]
pub struct Mask128(pub(crate) uint8x16_t);

impl Mask for Mask128 {
    type BitMask = NeonBits;
    type Element = u8;

    /// Convert Mask Vector 0x00-ff-ff to Bits 0b0000-1111-1111
    /// Reference: https://community.arm.com/arm-community-blogs/b/infrastructure-solutions-blog/posts/porting-x86-vector-bitmask-optimizations-to-arm-neon
    #[inline(always)]
    fn bitmask(self) -> Self::BitMask {
        unsafe {
            let v16 = vreinterpretq_u16_u8(self.0);
            let sr4 = vshrn_n_u16(v16, 4);
            let v64 = vreinterpret_u64_u8(sr4);
            NeonBits::new(vget_lane_u64(v64, 0))
        }
    }

    #[inline(always)]
    fn splat(b: bool) -> Self {
        let v: i8 = if b { -1 } else { 0 };
        unsafe { Self(vdupq_n_u8(v as u8)) }
    }
}

// Bitwise AND for Mask128
impl std::ops::BitAnd<Mask128> for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Mask128) -> Self::Output {
        unsafe { Self(vandq_u8(self.0, rhs.0)) }
    }
}

// Bitwise OR for Mask128
impl std::ops::BitOr<Mask128> for Mask128 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Mask128) -> Self::Output {
        unsafe { Self(vorrq_u8(self.0, rhs.0)) }
    }
}

// Bitwise OR assignment for Mask128
impl std::ops::BitOrAssign<Mask128> for Mask128 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Mask128) {
        unsafe {
            self.0 = vorrq_u8(self.0, rhs.0);
        }
    }
}

#[inline(always)]
pub unsafe fn to_bitmask64(v0: uint8x16_t, v1: uint8x16_t, v2: uint8x16_t, v3: uint8x16_t) -> u64 {
    let bit_mask = std::mem::transmute::<[u8; 16], uint8x16_t>(BIT_MASK_TAB);

    let t0 = vandq_u8(v0, bit_mask);
    let t1 = vandq_u8(v1, bit_mask);
    let t2 = vandq_u8(v2, bit_mask);
    let t3 = vandq_u8(v3, bit_mask);

    let pair0 = vpaddq_u8(t0, t1);
    let pair1 = vpaddq_u8(t2, t3);
    let quad = vpaddq_u8(pair0, pair1);
    let octa = vpaddq_u8(quad, quad);

    vgetq_lane_u64(vreinterpretq_u64_u8(octa), 0)
}

#[inline(always)]
pub(crate) unsafe fn to_bitmask32(v0: uint8x16_t, v1: uint8x16_t) -> u32 {
    let bit_mask = std::mem::transmute::<[u8; 16], uint8x16_t>(BIT_MASK_TAB);

    let t0 = vandq_u8(v0, bit_mask);
    let t1 = vandq_u8(v1, bit_mask);

    let pair = vpaddq_u8(t0, t1);
    let quad = vpaddq_u8(pair, pair);
    let octa = vpaddq_u8(quad, quad);

    vgetq_lane_u32(vreinterpretq_u32_u8(octa), 0)
}
