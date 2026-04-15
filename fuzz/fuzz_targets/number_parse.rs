//! Fuzz target for number parsing — exercises SWAR, unchecked parsing,
//! and boundary conditions (u64 overflow, f64 precision, exponent edges).
#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs_fuzz::gen::NumberInput;

fuzz_target!(|input: NumberInput| sonic_rs_fuzz::fuzz_number_input(&input));
