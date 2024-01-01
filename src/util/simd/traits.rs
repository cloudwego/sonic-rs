use std::ops::{BitAnd, BitOr, BitOrAssign};

/// Portbal SIMD traits
pub trait Simd: Sized {
    const LANES: usize;

    type Mask;

    unsafe fn from_slice_unaligned_unchecked(slice: &[u8]) -> Self {
        debug_assert!(slice.len() >= Self::LANES);
        Self::loadu(slice.as_ptr())
    }

    unsafe fn write_to_slice_unaligned_unchecked(&self, slice: &mut [u8]) {
        debug_assert!(slice.len() >= Self::LANES);
        self.storeu(slice.as_mut_ptr());
    }

    unsafe fn loadu(ptr: *const u8) -> Self;

    unsafe fn storeu(&self, ptr: *mut u8);

    fn eq(&self, lhs: &Self) -> Self::Mask;

    fn splat(ch: u8) -> Self;

    fn le(&self, lhs: &Self) -> Self::Mask;

    fn gt(&self, lhs: &Self) -> Self::Mask;
}

pub trait Mask: Sized + BitOr<Self> + BitOrAssign + BitAnd<Self> {
    type BitMap;

    fn bitmask(self) -> Self::BitMap;

    fn splat(b: bool) -> Self;
}
