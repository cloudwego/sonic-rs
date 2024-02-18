use std::ops::{BitAnd, BitOr, BitOrAssign};

/// Portbal SIMD traits
pub trait Simd: Sized {
    const LANES: usize;

    type Element;
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

    fn eq(&self, rhs: &Self) -> Self::Mask;

    fn splat(elem: Self::Element) -> Self;

    /// greater than
    fn gt(&self, rhs: &Self) -> Self::Mask;

    /// less or equal
    fn le(&self, rhs: &Self) -> Self::Mask;
}

/// Portbal SIMD mask traits
pub trait Mask: Sized + BitOr<Self> + BitOrAssign + BitAnd<Self> {
    type Element;
    type Bitmap;

    fn bitmask(self) -> Self::Bitmap;

    fn splat(b: bool) -> Self;
}

/// Trait for Bitmap.
pub trait BitMask {
    /// Total bits in the bitmap.
    const LEN: usize;

    /// get the offset of the first `1` bit.
    fn first_offset(&self) -> usize;

    /// check if this bitmap is before the other bitmap.
    fn before(&self, rhs: &Self) -> bool;

    /// convert bitmap as little endian
    fn as_little_endian(&self) -> Self;
}
