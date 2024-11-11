use std::ops::{BitAnd, BitOr, BitOrAssign};

/// Portbal SIMD traits
pub trait Simd: Sized {
    const LANES: usize;

    type Element;
    type Mask: Mask;

    /// # Safety
    unsafe fn from_slice_unaligned_unchecked(slice: &[u8]) -> Self {
        debug_assert!(slice.len() >= Self::LANES);
        Self::loadu(slice.as_ptr())
    }

    /// # Safety
    unsafe fn write_to_slice_unaligned_unchecked(&self, slice: &mut [u8]) {
        debug_assert!(slice.len() >= Self::LANES);
        self.storeu(slice.as_mut_ptr());
    }

    /// # Safety
    unsafe fn loadu(ptr: *const u8) -> Self;

    /// # Safety
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
    type BitMask: BitMask;

    fn bitmask(self) -> Self::BitMask;

    fn splat(b: bool) -> Self;
}

/// Trait for the bitmask of a vector Mask.
pub trait BitMask {
    /// Total bits in the bitmask.
    const LEN: usize;

    /// get the offset of the first `1` bit.
    fn first_offset(&self) -> usize;

    /// check if this bitmask is before the other bitmask.
    fn before(&self, rhs: &Self) -> bool;

    /// convert bitmask as little endian
    fn as_little_endian(&self) -> Self;

    /// whether all bits are zero.
    fn all_zero(&self) -> bool;

    /// clear high n bits.
    fn clear_high_bits(&self, n: usize) -> Self;
}
