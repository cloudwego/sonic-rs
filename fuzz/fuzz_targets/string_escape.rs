//! Fuzz target for string parsing — exercises escape sequences, unicode
//! handling, SIMD chunk boundaries, and in-place unescaping.
#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs_fuzz::gen::JsonValue;

fuzz_target!(|input: JsonValue| sonic_rs_fuzz::fuzz_string_value(&input));
