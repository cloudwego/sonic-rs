use super::traits::BitMask;

macro_rules! impl_bits {
    () => {};
    ($($ty:ty)*) => {
        $(
            impl BitMask for $ty {
                const LEN: usize = std::mem::size_of::<$ty>() * 8;

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
            }
        )*
    };
}

impl_bits!(u16 u32 u64);

pub struct NeonBits(u64);

impl NeonBits {
    #[inline]
    pub fn new(u: u64) -> Self {
        Self(u)
    }
}

impl BitMask for NeonBits {
    const LEN: usize = 8;

    /// reference: https://community.arm.com/arm-community-blogs/b/infrastructure-solutions-blog/posts/porting-x86-vector-bitmask-optimizations-to-arm-neon
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
