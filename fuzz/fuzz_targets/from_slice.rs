#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs_fuzz::sonic_rs_fuzz_data;

fuzz_target!(|data: &[u8]| sonic_rs_fuzz_data(data));
