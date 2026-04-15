//! Fuzz target for deep nesting — exercises NodeBuf/tls_buffer capacity,
//! stack usage, and pointer arithmetic at scale.
#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs_fuzz::gen::DeepNestInput;

fuzz_target!(|input: DeepNestInput| sonic_rs_fuzz::fuzz_deep_nesting_input(&input));
