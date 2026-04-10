//! Fuzz target for number parsing — exercises SWAR, unchecked parsing,
//! and boundary conditions (u64 overflow, f64 precision, exponent edges).
#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs::{JsonNumberTrait, JsonValueTrait};
use sonic_rs_fuzz::gen::NumberInput;

fuzz_target!(|input: NumberInput| {
    let json = input.to_json_bytes();

    // --- Strategy 1: Compare sonic-rs vs serde_json for type-level consistency ---
    macro_rules! cmp_number {
        ($ty:ty) => {
            match serde_json::from_slice::<$ty>(&json) {
                Ok(expected) => {
                    let got: $ty = sonic_rs::from_slice(&json).unwrap_or_else(|e| {
                        panic!(
                            "sonic-rs failed to parse {:?} as {}: {}",
                            std::str::from_utf8(&json).unwrap_or("<non-utf8>"),
                            stringify!($ty),
                            e
                        )
                    });
                    assert_eq!(
                        got, expected,
                        "mismatch for {} on {:?}",
                        stringify!($ty),
                        std::str::from_utf8(&json).unwrap_or("<non-utf8>")
                    );
                }
                Err(_) => {
                    // serde_json rejects it — sonic-rs should too
                    let _ = sonic_rs::from_slice::<$ty>(&json);
                }
            }
        };
    }

    cmp_number!(u8);
    cmp_number!(u16);
    cmp_number!(u32);
    cmp_number!(u64);
    cmp_number!(u128);
    cmp_number!(i8);
    cmp_number!(i16);
    cmp_number!(i32);
    cmp_number!(i64);
    cmp_number!(i128);
    cmp_number!(f32);
    cmp_number!(f64);

    // --- Strategy 2: Value-level number parsing ---
    if let Ok(sv) = sonic_rs::from_slice::<sonic_rs::Value>(&json) {
        if let Ok(jv) = serde_json::from_slice::<serde_json::Value>(&json) {
            if let (Some(sn), Some(jn)) = (sv.as_number(), jv.as_number()) {
                // Compare all numeric representations
                if jn.is_u64() {
                    assert_eq!(sn.as_u64(), jn.as_u64(), "u64 mismatch on {:?}", json);
                }
                if jn.is_i64() {
                    assert_eq!(sn.as_i64(), jn.as_i64(), "i64 mismatch on {:?}", json);
                }
                if jn.is_f64() {
                    assert_eq!(sn.as_f64(), jn.as_f64(), "f64 mismatch on {:?}", json);
                }
            }
        }
    }

    // --- Strategy 3: Round-trip consistency ---
    if let Ok(sv) = sonic_rs::from_slice::<sonic_rs::Value>(&json) {
        let serialized = sonic_rs::to_string(&sv).unwrap();
        let sv2: sonic_rs::Value = sonic_rs::from_str(&serialized).unwrap();
        // Numbers should round-trip
        if let Some(n1) = sv.as_number() {
            let n2 = sv2.as_number().expect("round-trip lost number type");
            assert_eq!(n1.as_f64(), n2.as_f64(), "f64 round-trip mismatch");
            assert_eq!(n1.as_u64(), n2.as_u64(), "u64 round-trip mismatch");
            assert_eq!(n1.as_i64(), n2.as_i64(), "i64 round-trip mismatch");
        }
    }

    // --- Strategy 4: Raw bytes fuzzing (non-structured) ---
    // Also test with the raw json bytes directly for the unchecked path
    if let Ok(s) = std::str::from_utf8(&json) {
        let _ = sonic_rs::from_str::<f64>(s);
        let _ = sonic_rs::from_str::<i64>(s);
        let _ = sonic_rs::from_str::<u64>(s);
        let _ = sonic_rs::from_str::<sonic_rs::Value>(s);
    }
});
