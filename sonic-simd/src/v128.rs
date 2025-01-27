use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::{Mask, Simd};

#[derive(Debug)]
pub struct Simd128i([i8; 16]);

#[derive(Debug)]
pub struct Simd128u([u8; 16]);

#[derive(Debug)]
pub struct Mask128([u8; 16]);

impl Simd for Simd128i {
    type Element = i8;
    const LANES: usize = 16;
    type Mask = Mask128;

    unsafe fn loadu(ptr: *const u8) -> Self {
        let v = std::slice::from_raw_parts(ptr, Self::LANES);
        let mut res = [0i8; 16];
        res.copy_from_slice(std::mem::transmute::<&[u8], &[i8]>(v));
        Self(res)
    }

    unsafe fn storeu(&self, ptr: *mut u8) {
        let data = std::mem::transmute::<&[i8], &[u8]>(&self.0);
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, Self::LANES);
    }

    fn eq(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for i in 0..Self::LANES {
            mask[i] = if self.0[i] == rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }

    fn splat(value: i8) -> Self {
        Self([value as i8; Self::LANES])
    }

    fn le(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for i in 0..Self::LANES {
            mask[i] = if self.0[i] <= rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }

    fn gt(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for i in 0..Self::LANES {
            mask[i] = if self.0[i] > rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }
}

impl Simd for Simd128u {
    type Element = u8;
    const LANES: usize = 16;
    type Mask = Mask128;

    unsafe fn loadu(ptr: *const u8) -> Self {
        let v = std::slice::from_raw_parts(ptr, Self::LANES);
        let mut res = [0u8; 16];
        res.copy_from_slice(v);
        Self(res)
    }

    unsafe fn storeu(&self, ptr: *mut u8) {
        let data = &self.0;
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, Self::LANES);
    }

    fn eq(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for i in 0..Self::LANES {
            mask[i] = if self.0[i] == rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }

    fn splat(value: u8) -> Self {
        Self([value; Self::LANES])
    }

    fn le(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for i in 0..Self::LANES {
            mask[i] = if self.0[i] <= rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }

    fn gt(&self, rhs: &Self) -> Self::Mask {
        let mut mask = [0u8; 16];
        for i in 0..Self::LANES {
            mask[i] = if self.0[i] > rhs.0[i] { 1 } else { 0 };
        }
        Mask128(mask)
    }
}

impl Mask for Mask128 {
    type BitMask = u16;
    type Element = u8;

    fn bitmask(self) -> Self::BitMask {
        #[cfg(target_endian = "little")]
        {
            self.0
                .iter()
                .enumerate()
                .fold(0, |acc, (i, &b)| acc | ((b as u16) << i))
        }
        #[cfg(target_endian = "big")]
        {
            self.0
                .iter()
                .enumerate()
                .fold(0, |acc, (i, &b)| acc | ((b as u16) << (15 - i)))
        }
    }

    fn splat(b: bool) -> Self {
        Mask128([b as u8; 16])
    }
}

impl BitAnd for Mask128 {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = self.0[i] & rhs.0[i];
        }
        Mask128(result)
    }
}

impl BitOr for Mask128 {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut result = [0u8; 16];
        for i in 0..16 {
            result[i] = self.0[i] | rhs.0[i];
        }
        Mask128(result)
    }
}

impl BitOrAssign for Mask128 {
    fn bitor_assign(&mut self, rhs: Self) {
        for i in 0..16 {
            self.0[i] |= rhs.0[i];
        }
    }
}
