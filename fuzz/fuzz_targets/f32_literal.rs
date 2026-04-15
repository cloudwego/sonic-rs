//! Fuzz target for direct f32 literal parsing against std's parser semantics.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    sonic_rs_fuzz::fuzz_f32_literal_bytes(data);
});
