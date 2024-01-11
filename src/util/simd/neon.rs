use std::{
    arch::aarch64::*,
    ops::{BitAnd, BitOr, BitOrAssign},
};

use crate::util::simd::{Mask, Simd};

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128u(uint8x16_t);

#[derive(Debug)]
#[repr(transparent)]
pub struct Simd128i(int8x16_t);

impl Simd for Simd128u {
    const LANES: usize = 16;
    type Mask = Mask128;

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
    fn splat(ch: u8) -> Self {
        unsafe { Self(vdupq_n_s8(ch as i8)) }
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
pub struct Mask128(uint8x16_t);

impl Mask for Mask128 {
    type BitMap = u16;

    #[inline(always)]
    fn bitmask(self) -> Self::BitMap {
        // TODO: optimize bitmask like this
        // neon doesn't have instruction same as movemask, to_bitmask uses shrn to
        // reduce 128bits -> 64bits. If a 128bits bool vector in x86 can convert
        // as 0101, neon shrn will convert it as 0000111100001111.
        // unsafe {
        //     vget_lane_u64(
        //         vreinterpret_u64_u8(vshrn_n_u16(vreinterpretq_u16_u8(self.0), 4)),
        //         0,
        //     ) as u16
        // }
        //
        unsafe {
            // Bit mask transmutation
            let bit_mask = std::mem::transmute(BIT_MASK_TAB);

            // Compute mask
            let input = vandq_u8(self.0, bit_mask);
            let pair = vpaddq_u8(input, input);
            let quad = vpaddq_u8(pair, pair);
            let octa = vpaddq_u8(quad, quad);

            // Extract and convert to u32
            let mask32 = vgetq_lane_u16(vreinterpretq_u16_u8(octa), 0);

            mask32 as u16
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
pub(crate) unsafe fn to_bitmask64(input: (uint8x16_t, uint8x16_t, uint8x16_t, uint8x16_t)) -> u64 {
    let bit_mask = std::mem::transmute(BIT_MASK_TAB);
    let (v0, v1, v2, v3): (uint8x16_t, uint8x16_t, uint8x16_t, uint8x16_t) = input;

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
pub(crate) unsafe fn to_bitmask32(input: (uint8x16_t, uint8x16_t)) -> u32 {
    let bit_mask = std::mem::transmute(BIT_MASK_TAB);
    let (v0, v1): (uint8x16_t, uint8x16_t) = input;

    let t0 = vandq_u8(v0, bit_mask);
    let t1 = vandq_u8(v1, bit_mask);

    let pair = vpaddq_u8(t0, t1);
    let quad = vpaddq_u8(pair, pair);
    let octa = vpaddq_u8(quad, quad);

    vgetq_lane_u32(vreinterpretq_u32_u8(octa), 0)
}
