mod arch;
mod common;
mod decimal;
mod float;
mod lemire;
mod slow;
mod table;

use self::{common::BiasedFp, float::RawFloat, table::POWER_OF_FIVE_128};
use crate::arch::simd_str2int;

const FLOATING_LONGEST_DIGITS: usize = 17;
const F64_BITS: u32 = 64;
const F64_SIG_BITS: u32 = 52;
const F64_SIG_FULL_BITS: u32 = 53;
const F64_EXP_BIAS: i32 = 1023;
const F64_SIG_MASK: u64 = 0x000F_FFFF_FFFF_FFFF;

#[derive(Debug)]
pub enum ParserNumber {
    Unsigned(u64),
    /// Always less than zero.
    Signed(i64),
    /// Always finite.
    Float(f64),
}

#[derive(Debug)]
pub enum Error {
    InvalidNumber,
    FloatMustBeFinite,
}

macro_rules! match_digit {
    ($data:expr, $i:expr, $pattern:pat) => {
        $i < $data.len() && matches!($data[$i], $pattern)
    };
}

macro_rules! is_digit {
    ($data:expr, $i:expr) => {
        $i < $data.len() && $data[$i].is_ascii_digit()
    };
}

macro_rules! digit {
    ($data:expr, $i:expr) => {
        ($data[$i] - b'0') as u64
    };
}

macro_rules! check_digit {
    ($data:expr, $i:expr) => {
        if !($i < $data.len() && $data[$i].is_ascii_digit()) {
            return Err(Error::InvalidNumber);
        }
    };
}

#[inline(always)]
fn parse_exponent(data: &[u8], index: &mut usize) -> Result<i32, Error> {
    let mut exponent: i32 = 0;
    let mut negative = false;

    if *index >= data.len() {
        return Err(Error::InvalidNumber);
    }

    match data[*index] {
        b'+' => *index += 1,
        b'-' => {
            negative = true;
            *index += 1;
        }
        _ => {}
    }

    check_digit!(data, *index);
    while exponent < 1000 && is_digit!(data, *index) {
        exponent = digit!(data, *index) as i32 + exponent * 10;
        *index += 1;
    }
    while is_digit!(data, *index) {
        *index += 1;
    }
    if negative {
        exponent = -exponent;
    }
    Ok(exponent)
}

const POW10_UINT: [u64; 18] = [
    1,
    10,
    100,
    1000,
    10000,
    100000,
    1000000,
    10000000,
    100000000,
    1000000000,
    10000000000,
    100000000000,
    1000000000000,
    10000000000000,
    100000000000000,
    1000000000000000,
    10000000000000000,
    100000000000000000,
];

// parse at most 16 digits for fraction, record the exponent.
// because we calcaute at least the first significant digit when both normal or subnormal float
// points
#[inline(always)]
fn parse_number_fraction(
    data: &[u8],
    index: &mut usize,
    significant: &mut u64,
    exponent: &mut i32,
    mut need: isize,
    dot_pos: usize,
) -> Result<bool, Error> {
    debug_assert!(need < FLOATING_LONGEST_DIGITS as isize);

    // native implement:
    // while need > 0 && is_digit!(data, *index) {
    //     *significant = *significant * 10 + digit!(data, *index);
    //     *index += 1;
    //     need -= 1;
    // }
    if need > 0 {
        if data.len() - *index >= 16 {
            let (frac, ndigits) = unsafe { simd_str2int(&data[*index..], need as usize) };
            *significant = *significant * POW10_UINT[ndigits] + frac;
            *index += ndigits;
        } else {
            while need > 0 && is_digit!(data, *index) {
                *significant = *significant * 10 + digit!(data, *index);
                *index += 1;
                need -= 1;
            }
        }
    }

    *exponent -= *index as i32 - dot_pos as i32;
    let mut trunc = false;
    while is_digit!(data, *index) {
        trunc = true;
        *index += 1;
    }

    if match_digit!(data, *index, b'e' | b'E') {
        *index += 1;
        *exponent += parse_exponent(data, &mut *index)?;
    }
    Ok(trunc)
}

#[inline(always)]
pub fn parse_number(data: &[u8], index: &mut usize, negative: bool) -> Result<ParserNumber, Error> {
    let mut significant: u64 = 0;
    let mut exponent: i32 = 0;
    let mut trunc = false;
    let raw_num = &data[*index..];

    if match_digit!(data, *index, b'0') {
        *index += 1;

        if *index >= data.len() || !matches!(data[*index], b'.' | b'e' | b'E') {
            // view -0 as float number
            if negative {
                return Ok(ParserNumber::Float(0.0));
            }
            return Ok(ParserNumber::Unsigned(0));
        }

        // deal with 0e123 or 0.000e123
        match data[*index] {
            b'.' => {
                *index += 1;
                let dot_pos = *index;
                check_digit!(data, *index);
                while match_digit!(data, *index, b'0') {
                    *index += 1;
                }
                // special case: 0.000e123
                if match_digit!(data, *index, b'e' | b'E') {
                    *index += 1;
                    if match_digit!(data, *index, b'-' | b'+') {
                        *index += 1;
                    }
                    check_digit!(data, *index);
                    while is_digit!(data, *index) {
                        *index += 1;
                    }
                    return Ok(ParserNumber::Float(0.0));
                }

                // we calculate the first digit here for two reasons:
                // 1. fastpath for small float number
                // 2. we only need parse at most 16 digits in parse_number_fraction
                // and it is friendly for simd
                if !is_digit!(data, *index) {
                    return Ok(ParserNumber::Float(0.0));
                }

                significant = digit!(data, *index);
                *index += 1;

                if is_digit!(data, *index) {
                    let need = FLOATING_LONGEST_DIGITS as isize - 1;
                    trunc = parse_number_fraction(
                        data,
                        index,
                        &mut significant,
                        &mut exponent,
                        need,
                        dot_pos,
                    )?;
                } else {
                    exponent -= *index as i32 - dot_pos as i32;
                    if match_digit!(data, *index, b'e' | b'E') {
                        *index += 1;
                        exponent += parse_exponent(data, &mut *index)?;
                    }
                }
            }
            b'e' | b'E' => {
                *index += 1;
                if match_digit!(data, *index, b'-' | b'+') {
                    *index += 1;
                }
                check_digit!(data, *index);
                while is_digit!(data, *index) {
                    *index += 1;
                }
                return Ok(ParserNumber::Float(0.0));
            }
            _ => unreachable!("unreachable branch in parse_number_unchecked"),
        }
    } else {
        // parse significant digits
        let digit_start = *index;
        while is_digit!(data, *index) {
            // assume most number is not overflow here. When it overflow, we will check digits count
            // and fallback into the slow path.
            significant = significant
                .wrapping_mul(10)
                .wrapping_add(digit!(data, *index));
            *index += 1;
        }
        let mut digits_cnt = *index - digit_start;
        if digits_cnt == 0 {
            return Err(Error::InvalidNumber);
        }

        // slow path for too long integer
        if digits_cnt > 19 {
            *index = digit_start;
            significant = 0;
            digits_cnt = 0;
            while is_digit!(data, *index) && digits_cnt < 19 {
                significant = significant * 10 + digit!(data, *index);
                digits_cnt += 1;
                *index += 1;
            }

            // overflow for u64 sig, mark as truncated
            while is_digit!(data, *index) {
                exponent += 1;
                *index += 1;
                trunc = true;
            }
        }

        // TODO: fix special case like `43332000001000000003888e-4`.
        // it should parse as `4.3332000001000003e18`.
        if match_digit!(data, *index, b'e' | b'E') {
            // parse exponent
            *index += 1;
            exponent += parse_exponent(data, index)?;
        } else if match_digit!(data, *index, b'.') {
            *index += 1;
            check_digit!(data, *index);
            let dot_pos = *index;

            // parse fraction
            let need = FLOATING_LONGEST_DIGITS as isize - digits_cnt as isize;
            trunc =
                parse_number_fraction(data, index, &mut significant, &mut exponent, need, dot_pos)?;
        } else {
            // parse integer, all parse has finished.
            if exponent == 0 {
                if negative {
                    if significant > (1u64 << 63) {
                        return Ok(ParserNumber::Float(-(significant as f64)));
                    } else {
                        // if significant is 0x8000_0000_0000_0000, it will overflow here.
                        // so, we must use wrapping_sub here.
                        return Ok(ParserNumber::Signed(0_i64.wrapping_sub(significant as i64)));
                    }
                } else {
                    return Ok(ParserNumber::Unsigned(significant));
                }
            } else if exponent == 1 {
                // now we get 20 digits, it maybe overflow for uint64
                let last = digit!(data, *index - 1);
                let (out, ov0) = significant.overflowing_mul(10);
                let (out, ov1) = out.overflowing_add(last);
                if !ov0 && !ov1 {
                    // negative must be overflow here.
                    significant = out;
                    if negative {
                        return Ok(ParserNumber::Float(-(significant as f64)));
                    } else {
                        return Ok(ParserNumber::Unsigned(significant));
                    }
                }
            }
            trunc = true;
        }
    }

    // raw_num is pass-through for fallback parsing logic
    parse_float(significant, exponent, negative, trunc, raw_num)
}

#[inline(always)]
fn parse_float(
    significant: u64,
    exponent: i32,
    negative: bool,
    trunc: bool,
    raw_num: &[u8],
) -> Result<ParserNumber, Error> {
    // parse double fast
    if significant >> 52 == 0 && (-22..=(22 + 15)).contains(&exponent) {
        if let Some(mut float) = parse_float_fast(exponent, significant) {
            if negative {
                float = -float;
            }
            return Ok(ParserNumber::Float(float));
        }
    }

    if !trunc && exponent > (-308 + 1) && exponent < (308 - 20) {
        if let Some(raw) = parse_floating_normal_fast(exponent, significant) {
            let mut float = f64::from_u64_bits(raw);
            if negative {
                float = -float;
            }
            return Ok(ParserNumber::Float(float));
        }
    }

    // If significant digits were truncated, then we can have rounding error
    // only if `mantissa + 1` produces a different result. We also avoid
    // redundantly using the Eisel-Lemire algorithm if it was unable to
    // correctly round on the first pass.
    let exponent = exponent as i64;
    let mut fp = lemire::compute_float::<f64>(exponent, significant);
    if trunc && fp.e >= 0 && fp != lemire::compute_float::<f64>(exponent, significant + 1) {
        fp.e = -1;
    }

    // Unable to correctly round the float using the Eisel-Lemire algorithm.
    // Fallback to a slower, but always correct algorithm.
    if fp.e < 0 {
        fp = slow::parse_long_mantissa::<f64>(raw_num);
    }

    let mut float = biased_fp_to_float::<f64>(fp);
    if negative {
        float = -float;
    }

    // check inf for float
    if float.is_infinite() {
        return Err(Error::FloatMustBeFinite);
    }
    Ok(ParserNumber::Float(float))
}

// This function is modified from yyjson
#[inline(always)]
fn parse_floating_normal_fast(exp10: i32, man: u64) -> Option<u64> {
    let (mut hi, lo, hi2, add, bits);
    let mut exp2: i32;
    let mut exact = false;
    let idx = exp10 + 342;
    let sig2_ext = POWER_OF_FIVE_128[idx as usize].1;
    let sig2 = POWER_OF_FIVE_128[idx as usize].0;

    let mut lz = man.leading_zeros();
    let sig1 = man << lz;
    exp2 = ((217706 * exp10 - 4128768) >> 16) - lz as i32;

    (lo, hi) = lemire::full_multiplication(sig1, sig2);

    bits = hi & ((1u64 << (64 - 54 - 1)) - 1);
    if bits.wrapping_sub(1) < ((1u64 << (64 - 54 - 1)) - 2) {
        exact = true;
    } else {
        (_, hi2) = lemire::full_multiplication(sig1, sig2_ext);
        // not need warring overflow here
        add = lo.wrapping_add(hi2);
        if add + 1 > 1u64 {
            let carry = add < lo || add < hi2;
            hi += carry as u64;
            exact = true;
        }
    }

    if exact {
        lz = if hi < (1u64 << 63) { 1 } else { 0 };
        hi <<= lz;
        exp2 -= lz as i32;
        exp2 += 64;

        let round_up = (hi & (1u64 << (64 - 54))) > 0;
        hi = hi.wrapping_add(if round_up { 1u64 << (64 - 54) } else { 0 });

        if hi < (1u64 << (64 - 54)) {
            hi = 1u64 << 63;
            exp2 += 1;
        }

        hi >>= F64_BITS - F64_SIG_FULL_BITS;
        exp2 += F64_BITS as i32 - F64_SIG_FULL_BITS as i32 + F64_SIG_BITS as i32;
        exp2 += F64_EXP_BIAS;
        let raw = ((exp2 as u64) << F64_SIG_BITS) | (hi & F64_SIG_MASK);
        return Some(raw);
    }
    None
}

#[inline(always)]
/// Converts a `BiasedFp` to the closest machine float type.
fn biased_fp_to_float<T: RawFloat>(x: BiasedFp) -> T {
    let mut word = x.f;
    word |= (x.e as u64) << T::MANTISSA_EXPLICIT_BITS;
    T::from_u64_bits(word)
}

#[inline(always)]
fn parse_float_fast(exp10: i32, significant: u64) -> Option<f64> {
    let mut d = significant as f64;
    if exp10 > 0 {
        if exp10 > 22 {
            d *= POW10_FLOAT[exp10 as usize - 22];
            if (-1e15..=1e15).contains(&d) {
                Some(d * POW10_FLOAT[22])
            } else {
                None
            }
        } else {
            Some(d * POW10_FLOAT[exp10 as usize])
        }
    } else {
        Some(d / POW10_FLOAT[(-exp10) as usize])
    }
}

const POW10_FLOAT: [f64; 23] = [
    /* <= the connvertion to double is not exact when less than 1 => */ 1e-000, 1e+001,
    1e+002, 1e+003, 1e+004, 1e+005, 1e+006, 1e+007, 1e+008, 1e+009, 1e+010, 1e+011, 1e+012, 1e+013,
    1e+014, 1e+015, 1e+016, 1e+017, 1e+018, 1e+019, 1e+020, 1e+021,
    1e+022, /* <= the connvertion to double is not exact when larger,  => */
];

#[cfg(test)]
mod test {
    use crate::{parse_number, ParserNumber};

    fn test_parse_ok(input: &str, expect: f64) {
        assert_eq!(input.parse::<f64>().unwrap(), expect);

        let mut data = input.as_bytes().to_vec();
        data.push(b' ');
        let mut index = 0;
        let num = parse_number(&data, &mut index, false).unwrap();
        assert!(
            matches!(num, ParserNumber::Float(f) if f == expect),
            "parsed is {:?} failed num is {}",
            num,
            input
        );
        assert_eq!(data[index], b' ', "failed num is {}", input);
    }

    #[test]
    fn test_parse_float() {
        test_parse_ok("0.0", 0.0);
        test_parse_ok("0.01", 0.01);
        test_parse_ok("0.1", 0.1);
        test_parse_ok("0.12", 0.12);
        test_parse_ok("0.123", 0.123);
        test_parse_ok("0.1234", 0.1234);
        test_parse_ok("0.12345", 0.12345);
        test_parse_ok("0.123456", 0.123456);
        test_parse_ok("0.1234567", 0.1234567);
        test_parse_ok("0.12345678", 0.12345678);
        test_parse_ok("0.123456789", 0.123456789);
        test_parse_ok("0.1234567890", 0.1234567890);
        test_parse_ok("0.10000000149011612", 0.10000000149011612);
        test_parse_ok("0.06411743306171047", 0.06411743306171047);

        test_parse_ok("0e-1", 0e-1);
        test_parse_ok("0e+1000000", 0e+1000000);
        test_parse_ok("0.001e-1", 0.001e-1);
        test_parse_ok("0.001e+123", 0.001e+123);
        test_parse_ok(
            "0.000000000000000000000000001e+123",
            0.000000000000000000000000001e+123,
        );

        test_parse_ok("1.0", 1.0);
        test_parse_ok("1350.0", 1350.0);
        test_parse_ok("1.10000000149011612", 1.1000000014901161);

        test_parse_ok("1e0", 1e0);
        test_parse_ok("1.0e0", 1.0e0);
        test_parse_ok("1.0e+0", 1.0e+0);
        test_parse_ok("1.001e-123", 1.001e-123);
        test_parse_ok("10000000149011610000.0e-123", 1.000_000_014_901_161e-104);
        test_parse_ok(
            "10000000149011612123.001e-123",
            1.000_000_014_901_161_2e-104,
        );
        test_parse_ok("33333333333333333333", 3.333333333333333e19);
        test_parse_ok("135e-12", 135e-12);

        // test truncated float number without dot
        test_parse_ok("12448139190673828122020e-47", 1.244813919067383e-25);
        test_parse_ok(
            "3469446951536141862700000000000000000e-62",
            3.469446951536142e-26,
        );
    }
}
