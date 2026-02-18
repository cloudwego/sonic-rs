use core::arch::aarch64::*;

/// we use unzip to extract even and odd indexed digits, then we can do multiplication and addition
/// in parallel. for [1, 2, 3, 4, 5, 6, 7, 8, 9, 5, 4, 3, 2, 1, 0, 0]
/// it will be split into even = [1, 3, 5, 7, 9, 4, 2, 0, ...] and odd = [2, 4, 6, 8, 5, 3, 1, 0,
/// ...] then we can compute 1*10 + 2, 3*10 + 4, 5*10 + 6, 7*10 + 8, 9*10 + 5, 4*10 + 3, 2*10 + 1,
/// 0*10 + 0 in parallel. so we get [12, 34, 56, 78, 95, 43, 21, 0]
macro_rules! packadd_1 {
    ($v:expr) => {
        unsafe {
            let even = vuzp1q_u8($v, $v);
            let odd = vuzp2q_u8($v, $v);
            vaddw_u8(vmull_u8(vget_low_u8(even), vdup_n_u8(10)), vget_low_u8(odd))
        }
    };
}

/// should be called after packadd_1
/// for [1, 2, 3, 4, 5, 6, 7, 8, 9, 5, 4, 3, 2, 1, 0, 0]
/// we get [12, 34, 56, 78, 95, 43, 21, 0] after packadd_1
/// here, it will be split into even = [12, 56, 95, 21] and odd = [34, 78, 43, 0]
/// then we can compute 12*100 + 34, 56*100 + 78, 95*100 + 43, 21*100 + 0 in parallel.
/// so we get [1234, 5678, 9543, 2100]
macro_rules! packadd_2 {
    ($v:expr) => {
        unsafe {
            let even = vuzp1q_u16($v, $v);
            let odd = vuzp2q_u16($v, $v);
            vaddw_u16(vmull_n_u16(vget_low_u16(even), 100), vget_low_u16(odd))
        }
    };
}

/// should be called after packadd_2, it will compute 4 digits in parallel
/// for [1, 2, 3, 4, 5, 6, 7, 8, 9, 5, 4, 3, 2, 1, 0, 0]
/// we get [1234, 5678, 9543, 2100] after packadd_2
/// here, it will be split into even = [1234, 9543] and odd = [5678, 2100]
/// then we can compute 1234*10000 + 5678 and 9543*10000 + 2100 in parallel,
/// so we get [12345678, 95432100]
macro_rules! packadd_4 {
    ($v:expr) => {
        unsafe {
            let even = vuzp1q_u32($v, $v);
            let odd = vuzp2q_u32($v, $v);
            vaddw_u32(vmull_n_u32(vget_low_u32(even), 10000), vget_low_u32(odd))
        }
    };
}

macro_rules! simd_add_5_8 {
    ($v:ident, $count:literal) => {{
        let shifted = vextq_u8::<$count>(vdupq_n_u8(0), $v);
        let p1 = packadd_1!(shifted);
        let p2 = packadd_2!(p1);
        (vgetq_lane_u32::<2>(p2) as u64) * 10000 + (vgetq_lane_u32::<3>(p2) as u64)
    }};
}

macro_rules! simd_add_8 {
    ($v:ident) => {{
        let p1 = packadd_1!($v);
        let p2 = packadd_2!(p1);
        packadd_4!(p2)
    }};
}

/// how it works:
/// for "123456789", we have [1, 2, 3, 4, 5, 6, 7, 8, 9, ...]
/// calling `vextq_u8::<N>` will keep N bytes of the original vector and align them to the right,
/// and fill the left with zeros. so we get
/// shift = [0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9] (16 - N zeros)
/// then its aligned to the right, we could call simd_add_8 to get [12345678, 9, ..]
/// and extract the first and second lane to get the final result.
macro_rules! simd_add_9_15 {
    ($v:ident, $count:literal) => {{
        let shifted = vextq_u8::<$count>(vdupq_n_u8(0), $v);
        let p4 = simd_add_8!(shifted);
        vgetq_lane_u64::<0>(p4) * 100000000 + vgetq_lane_u64::<1>(p4)
    }};
}

// 4096 is a conservative page size
// which should work for most platforms
#[inline(always)]
fn is_page_safe(ptr: *const u8) -> bool {
    // we use 128bit load, which is 16 bytes
    ((ptr as usize) & 0xFFF) <= (4096 - 16)
}

#[inline(always)]
pub unsafe fn simd_str2int(c: &[u8], need: usize) -> (u64, usize) {
    debug_assert!(need <= 16);

    if !is_page_safe(c.as_ptr()) {
        let mut sum = 0u64;
        let mut i = 0;
        while i < need && c.get_unchecked(i).is_ascii_digit() {
            sum = (c.get_unchecked(i) - b'0') as u64 + sum * 10;
            i += 1;
        }
        return (sum, i);
    }

    let data = vld1q_u8(c.as_ptr());
    let zero_char = vdupq_n_u8(b'0');

    let digits = vsubq_u8(data, zero_char);
    let gt_nine = vcgtq_u8(digits, vdupq_n_u8(9));

    let mask16 = vreinterpretq_u16_u8(gt_nine);
    let mask8 = vshrn_n_u16::<4>(mask16);
    let mask64 = vget_lane_u64::<0>(vreinterpret_u64_u8(mask8));

    let mut count = need;
    if mask64 != 0 {
        let parsed_digits = (mask64.trailing_zeros() >> 2) as usize;
        if parsed_digits < need {
            count = parsed_digits;
        }
    }

    let sum = match count {
        0 => 0,
        1 => vgetq_lane_u8::<0>(digits) as u64,
        2 => (vgetq_lane_u8::<0>(digits) as u64) * 10 + (vgetq_lane_u8::<1>(digits) as u64),
        3 => {
            let shifted = vextq_u8::<3>(vdupq_n_u8(0), digits);
            let p1 = packadd_1!(shifted);
            (vgetq_lane_u16::<6>(p1) as u64) * 100 + (vgetq_lane_u16::<7>(p1) as u64)
        }
        4 => {
            let shifted = vextq_u8::<4>(vdupq_n_u8(0), digits);
            let p1 = packadd_1!(shifted);
            (vgetq_lane_u16::<6>(p1) as u64) * 100 + (vgetq_lane_u16::<7>(p1) as u64)
        }
        5 => simd_add_5_8!(digits, 5),
        6 => simd_add_5_8!(digits, 6),
        7 => simd_add_5_8!(digits, 7),
        8 => simd_add_5_8!(digits, 8),
        9 => simd_add_9_15!(digits, 9),
        10 => simd_add_9_15!(digits, 10),
        11 => simd_add_9_15!(digits, 11),
        12 => simd_add_9_15!(digits, 12),
        13 => simd_add_9_15!(digits, 13),
        14 => simd_add_9_15!(digits, 14),
        15 => simd_add_9_15!(digits, 15),
        16 => {
            let p = simd_add_8!(digits);
            vgetq_lane_u64::<0>(p) * 100000000 + vgetq_lane_u64::<1>(p)
        }
        _ => core::hint::unreachable_unchecked(),
    };

    (sum, count)
}
