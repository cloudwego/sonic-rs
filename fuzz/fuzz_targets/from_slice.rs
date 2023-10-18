#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(dom) = sonic_rs::value::dom_from_slice(data) {
        let _ = sonic_rs::to_string(&dom).unwrap();
    }
});
