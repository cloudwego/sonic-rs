//! Fuzz target for string parsing — exercises escape sequences, unicode
//! handling, SIMD chunk boundaries, and in-place unescaping.
#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs::JsonValueTrait;
use sonic_rs_fuzz::gen::JsonValue;

fuzz_target!(|input: JsonValue| {
    let json = input.to_json();
    let json_bytes = json.as_bytes();

    // --- Strategy 1: Parse and compare with serde_json ---
    match serde_json::from_str::<serde_json::Value>(&json) {
        Ok(jv) => {
            let sv: sonic_rs::Value = sonic_rs::from_str(&json).unwrap_or_else(|e| {
                panic!("sonic-rs failed on valid JSON: {}\njson: {}", e, &json[..json.len().min(200)])
            });

            // Use the existing compare_value from fuzz lib (handles duplicate keys)
            sonic_rs_fuzz::compare_value(&jv, &sv);

            // Round-trip: serialize and re-parse
            let out = sonic_rs::to_string(&sv).unwrap();
            let sv2: sonic_rs::Value = sonic_rs::from_str(&out).unwrap();
            let jv2: serde_json::Value = serde_json::from_str(&out).unwrap();
            sonic_rs_fuzz::compare_value(&jv2, &sv2);
        }
        Err(_) => {
            // serde_json rejects — sonic-rs should also reject
            let _ = sonic_rs::from_str::<sonic_rs::Value>(&json);
        }
    }

    // --- Strategy 2: String type deserialization ---
    if let Ok(expected) = serde_json::from_str::<String>(&json) {
        let got: String = sonic_rs::from_str(&json).unwrap_or_else(|e| {
            panic!("sonic-rs String deser failed: {}\njson: {}", e, &json[..json.len().min(200)])
        });
        assert_eq!(got, expected, "String mismatch on: {}", &json[..json.len().min(200)]);
    }

    // --- Strategy 3: UTF-8 lossy mode ---
    if let Ok(_) = sonic_rs::from_slice::<sonic_rs::Value>(json_bytes) {
        let json_str = unsafe { std::str::from_utf8_unchecked(json_bytes) };
        let mut de = sonic_rs::Deserializer::from_str(json_str).utf8_lossy();
        let _: Result<sonic_rs::Value, _> = serde::Deserialize::deserialize(&mut de);
    }

    // --- Strategy 4: LazyValue string access ---
    if let Ok(jv) = serde_json::from_str::<serde_json::Value>(&json) {
        if jv.is_string() {
            if let Ok(lv) = sonic_rs::from_str::<sonic_rs::LazyValue>(&json) {
                let raw = lv.as_raw_str();
                // Re-parse the raw string
                let sv: sonic_rs::Value = sonic_rs::from_str(raw).unwrap();
                assert!(sv.is_str());
                assert_eq!(sv.as_str().unwrap(), jv.as_str().unwrap());
            }
        }
    }
});

