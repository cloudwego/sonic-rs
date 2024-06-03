use serde::de;

use super::traits::BitMask;

macro_rules! impl_bits {
    () => {};
    ($($ty:ty)*) => {
        $(
            impl BitMask for $ty {
                const LEN: usize = std::mem::size_of::<$ty>() * 8;

                type Primitive = $ty;

                #[inline]
                fn as_primitive(&self) -> $ty {
                    *self
                }

                #[inline]
                fn before(&self, rhs: &Self) -> bool {
                    (self.as_little_endian()  & rhs.as_little_endian().wrapping_sub(1)) != 0
                }

                #[inline]
                fn first_offset(&self) -> usize {
                    self.as_little_endian().trailing_zeros() as usize
                }

                #[inline]
                fn as_little_endian(&self) -> Self {
                    #[cfg(target_endian = "little")]
                    {
                        self.clone()
                    }
                    #[cfg(target_endian = "big")]
                    {
                        self.swap_bytes()
                    }
                }

                #[inline]
                fn all_zero(&self) -> bool {
                    *self == 0
                }

                #[inline]
                fn clear_high_bits(&self, n: usize) -> Self {
                    debug_assert!(n <= Self::LEN);
                    *self & ((u64::MAX as $ty) >> n)
                }
            }
        )*
    };
}

impl_bits!(u16 u32 u64);

/// Use u64 representation the bitmask of Neon vector.
///         (low)
/// Vector: 00-ff-ff-ff-ff-00-00-00
/// Mask  : 0000-1111-1111-1111-1111-0000-0000-0000
///
/// first_offset() = 1
/// clear_high_bits(4) = Mask(0000-1111-1111-1111-[0000]-0000-0000-0000)
///
/// reference: https://community.arm.com/arm-community-blogs/b/infrastructure-solutions-blog/posts/porting-x86-vector-bitmask-optimizations-to-arm-neon
pub struct NeonBits(u64);

impl NeonBits {
    #[inline]
    pub fn new(u: u64) -> Self {
        Self(u)
    }
}

impl BitMask for NeonBits {
    const LEN: usize = 16;

    type Primitive = u64;

    #[inline]
    fn as_primitive(&self) -> Self::Primitive {
        self.0
    }

    #[inline]
    fn first_offset(&self) -> usize {
        (self.as_little_endian().0.trailing_zeros() as usize) >> 2
    }

    #[inline]
    fn before(&self, rhs: &Self) -> bool {
        (self.as_little_endian().0 & rhs.as_little_endian().0.wrapping_sub(1)) != 0
    }

    #[inline]
    fn as_little_endian(&self) -> Self {
        #[cfg(target_endian = "little")]
        {
            Self::new(self.0)
        }
        #[cfg(target_endian = "big")]
        {
            Self::new(self.0.swap_bytes())
        }
    }

    #[inline]
    fn all_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    fn clear_high_bits(&self, n: usize) -> Self {
        debug_assert!(n <= Self::LEN);
        Self(self.0 & u64::MAX >> (n * 4))
    }
}

pub fn combine_u16(lo: u16, hi: u16) -> u32 {
    #[cfg(target_endian = "little")]
    {
        (lo as u32) | ((hi as u32) << 16)
    }
    #[cfg(target_endian = "big")]
    {
        (hi as u32) | ((lo as u32) << 16)
    }
}

pub fn combine_u32(lo: u32, hi: u32) -> u64 {
    #[cfg(target_endian = "little")]
    {
        (lo as u64) | ((hi as u64) << 32)
    }
    #[cfg(target_endian = "big")]
    {
        (hi as u64) | ((lo as u64) << 32)
    }
}
