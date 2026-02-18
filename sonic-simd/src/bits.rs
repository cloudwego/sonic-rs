use super::traits::BitMask;

macro_rules! impl_bits {
    () => {};
    ($($ty:ty)*) => {
        $(
            impl BitMask for $ty {
                const LEN: usize = core::mem::size_of::<$ty>() * 8;

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

#[cfg(target_feature = "sve2")]
#[derive(Debug, Clone, Copy)]
pub struct SveBits(usize);

#[cfg(target_feature = "sve2")]
impl SveBits {
    #[inline(always)]
    pub fn new(u: usize) -> Self {
        Self(u)
    }
}

#[cfg(target_feature = "sve2")]
impl BitMask for SveBits {
    const LEN: usize = 16;

    #[inline(always)]
    fn first_offset(&self) -> usize {
        self.0
    }

    #[inline(always)]
    fn before(&self, rhs: &Self) -> bool {
        self.0 < rhs.0
    }

    #[inline(always)]
    fn all_zero(&self) -> bool {
        self.0 == 16
    }

    #[inline(always)]
    fn as_little_endian(&self) -> Self {
        *self
    }

    #[inline(always)]
    fn clear_high_bits(&self, n: usize) -> Self {
        let nb = 16 - n;

        if self.0 >= nb {
            Self(16)
        } else {
            *self
        }
    }
}

#[cfg(test)]
#[cfg(target_feature = "sve2")]
#[cfg(target_arch = "aarch64")]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct SVEStringBlock {
        bs_bits: SveBits,
        quote_bits: SveBits,
        unescaped_bits: SveBits,
    }

    impl SVEStringBlock {
        #[inline(always)]
        pub fn new_sve(ptr: *const u8) -> Self {
            let (q, bs, un): (u64, u64, u64);

            unsafe {
                core::arch::asm!(
                    "ptrue p0.b, vl16",
                    "ld1b {{z0.b}}, p0/z, [{ptr}]",

                    // "
                    "mov z1.b, #34",
                    "match p1.b, p0/z, z0.b, z1.b",
                    "brkb  p1.b, p0/z, p1.b",
                    "cntp  {q_idx}, p0, p1.b",

                    // /
                    "mov z1.b, #92",
                    "match p1.b, p0/z, z0.b, z1.b",
                    "brkb  p1.b, p0/z, p1.b",
                    "cntp  {bs_idx}, p0, p1.b",

                    // ascii control characters
                    "mov z1.b, #31",
                    "cmple p1.b, p0/z, z0.b, z1.b",
                    "brkb  p1.b, p0/z, p1.b",
                    "cntp  {un_idx}, p0, p1.b",

                    ptr = in(reg) ptr,
                    q_idx = out(reg) q,
                    bs_idx = out(reg) bs,
                    un_idx = out(reg) un,
                    out("z0") _, out("z1") _,
                    out("p0") _, out("p1") _,
                );
            }

            Self {
                quote_bits: SveBits::new(q as usize),
                bs_bits: SveBits::new(bs as usize),
                unescaped_bits: SveBits::new(un as usize),
            }
        }
    }

    impl SVEStringBlock {
        #[inline(always)]
        pub fn has_unescaped(&self) -> bool {
            self.unescaped_bits.0 < self.quote_bits.0
        }

        #[inline(always)]
        pub fn has_quote_first(&self) -> bool {
            self.quote_bits.0 < self.bs_bits.0 && !self.has_unescaped()
        }

        #[inline(always)]
        pub fn has_backslash(&self) -> bool {
            self.bs_bits.0 < self.quote_bits.0
        }

        #[inline(always)]
        pub fn quote_index(&self) -> usize {
            self.quote_bits.0
        }
    }

    #[test]
    fn test_sve_bits() {
        let s = b"\"\\\t\n";
        let block = SVEStringBlock::new_sve(s.as_ptr());
        assert_eq!(block.quote_bits.0, 0);
        assert_eq!(block.bs_bits.0, 1);
        assert_eq!(block.unescaped_bits.0, 2);

        let block = SVEStringBlock::new_sve(unsafe {
            {
                s.as_ptr().add(2)
            }
        });
        assert_eq!(block.quote_bits.0, 16);
        assert_eq!(block.bs_bits.0, 16);
        assert_eq!(block.unescaped_bits.0, 0);
    }
}
