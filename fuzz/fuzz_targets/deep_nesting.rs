//! Fuzz target for deep nesting — exercises NodeBuf/tls_buffer capacity,
//! stack usage, and pointer arithmetic at scale.
#![no_main]

use libfuzzer_sys::fuzz_target;
use sonic_rs_fuzz::gen::DeepNestInput;

fuzz_target!(|input: DeepNestInput| {
    let json = input.to_json();
    let json_bytes = json.as_bytes();

    // --- Strategy 1: Parse as Value (exercises NodeBuf allocation) ---
    let sv_result = sonic_rs::from_str::<sonic_rs::Value>(&json);
    let jv_result = serde_json::from_str::<serde_json::Value>(&json);

    match (&jv_result, &sv_result) {
        (Ok(jv), Ok(sv)) => {
            sonic_rs_fuzz::compare_value(jv, sv);

            // Round-trip
            let out = sonic_rs::to_string(sv).unwrap();
            let sv2: sonic_rs::Value = sonic_rs::from_str(&out).unwrap();
            let jv2: serde_json::Value = serde_json::from_str(&out).unwrap();
            sonic_rs_fuzz::compare_value(&jv2, &sv2);
        }
        (Err(_), Err(_)) => {} // Both reject — OK (e.g., too deep)
        (Ok(_), Err(e)) => {
            panic!(
                "sonic-rs rejected valid deep JSON: {}\njson len: {}",
                e,
                json.len()
            );
        }
        (Err(_), Ok(_)) => {
            // sonic-rs accepted something serde_json rejected — may be depth limit difference
        }
    }

    // --- Strategy 2: Parse as OwnedLazyValue (different allocation path) ---
    if jv_result.is_ok() {
        if let Ok(olv) = sonic_rs::from_str::<sonic_rs::OwnedLazyValue>(&json) {
            // Walk the structure to exercise recursive access
            walk_owned_lazy(&olv, 0);
        }
    }

    // --- Strategy 3: Lazy iteration on deep structures ---
    if let Ok(jv) = &jv_result {
        if jv.is_array() {
            for ret in sonic_rs::to_array_iter(json_bytes) {
                let lv = ret.unwrap();
                // Re-parse each element
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            }
        } else if jv.is_object() {
            for ret in sonic_rs::to_object_iter(json_bytes) {
                let (_, lv) = ret.unwrap();
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            }
        }
    }

    // --- Strategy 4: from_slice (exercises PaddedSliceRead path) ---
    if jv_result.is_ok() {
        let sv: sonic_rs::Value = sonic_rs::from_slice(json_bytes).unwrap();
        let out = sonic_rs::to_string(&sv).unwrap();
        let _ = sonic_rs::from_str::<sonic_rs::Value>(&out).unwrap();
    }
});

/// Recursively walk an OwnedLazyValue to exercise deep access patterns.
fn walk_owned_lazy(v: &sonic_rs::OwnedLazyValue, depth: usize) {
    use sonic_rs::JsonValueTrait;

    // Limit recursion to avoid stack overflow in the fuzzer itself
    if depth > 600 {
        return;
    }

    if v.is_object() {
        // Try index-based access with known key
        if let Some(child) = v.get("k") {
            walk_owned_lazy(child, depth + 1);
        }
    } else if v.is_array() {
        // Walk first few elements by index
        for i in 0..16 {
            match v.get(i) {
                Some(child) => walk_owned_lazy(child, depth + 1),
                None => break,
            }
        }
    } else if v.is_str() {
        let _ = v.as_str();
    } else if v.is_number() {
        let _ = v.as_f64();
        let _ = v.as_i64();
        let _ = v.as_u64();
    }
}
