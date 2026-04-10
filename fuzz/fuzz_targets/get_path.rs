//! Fuzz target for JSON path access — exercises `get()`, `get_unchecked()`,
//! `to_object_iter()`, `to_array_iter()` with structured and random paths.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use sonic_rs_fuzz::gen::JsonValue;

/// Fuzz input: a structured JSON value + a set of paths to probe.
#[derive(Debug, Arbitrary)]
struct GetPathInput {
    /// The JSON document to query.
    value: JsonValue,
    /// Path components to use for get() calls.
    paths: Vec<PathComponent>,
}

#[derive(Debug, Arbitrary)]
enum PathComponent {
    Key(SmallString),
    Index(u16),
}

#[derive(Debug)]
struct SmallString(String);

impl<'a> Arbitrary<'a> for SmallString {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let len = u.int_in_range(0..=16)?;
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            let c = u.int_in_range(b'a'..=b'z')?;
            s.push(c as char);
        }
        Ok(SmallString(s))
    }
}

fuzz_target!(|input: GetPathInput| {
    let json = input.value.to_json();
    let json_bytes = json.as_bytes();

    // Only proceed if it's valid JSON (our generator should always produce valid JSON)
    let jv: serde_json::Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(_) => return,
    };

    let _sv: sonic_rs::Value = match sonic_rs::from_str(&json) {
        Ok(v) => v,
        Err(e) => panic!("sonic-rs rejected valid JSON: {}\njson: {}", e, &json[..json.len().min(200)]),
    };

    // --- Strategy 1: Single-level get() with string keys ---
    if jv.is_object() {
        for path in &input.paths {
            if let PathComponent::Key(SmallString(key)) = path {
                let jresult = jv.get(key.as_str());
                let sresult = sonic_rs::get(json_bytes, &[key.as_str()]);

                match (jresult, sresult) {
                    (Some(_jv), Ok(slv)) => {
                        // Both found — verify value consistency
                        let reparsed: sonic_rs::Value =
                            sonic_rs::from_str(slv.as_raw_str()).unwrap();
                        let jreparsed: serde_json::Value =
                            serde_json::from_str(slv.as_raw_str()).unwrap();
                        sonic_rs_fuzz::compare_value(&jreparsed, &reparsed);
                    }
                    (None, Err(_)) => {} // Both not found — OK
                    (None, Ok(_)) => {
                        // sonic-rs found something serde_json didn't — possible with repeated keys
                    }
                    (Some(_), Err(e)) => {
                        panic!(
                            "sonic-rs get() failed but serde_json found key {:?}: {}\njson: {}",
                            key, e, &json[..json.len().min(200)]
                        );
                    }
                }

                // Also test unchecked variant on valid JSON
                unsafe {
                    if let Ok(slv) = sonic_rs::get_unchecked(json_bytes, &[key.as_str()]) {
                        let _ = sonic_rs::from_str::<sonic_rs::Value>(slv.as_raw_str()).unwrap();
                    }
                }
            }
        }
    }

    // --- Strategy 2: Array index access ---
    if jv.is_array() {
        for path in &input.paths {
            if let PathComponent::Index(idx) = path {
                let idx = *idx as usize;
                let jresult = jv.get(idx);
                let sresult = sonic_rs::get(json_bytes, [idx]);

                match (jresult, sresult) {
                    (Some(_), Ok(slv)) => {
                        let reparsed: sonic_rs::Value =
                            sonic_rs::from_str(slv.as_raw_str()).unwrap();
                        let jreparsed: serde_json::Value =
                            serde_json::from_str(slv.as_raw_str()).unwrap();
                        sonic_rs_fuzz::compare_value(&jreparsed, &reparsed);
                    }
                    (None, Err(_)) => {}
                    (Some(_), Err(e)) => {
                        panic!(
                            "sonic-rs get([{}]) failed but serde_json found: {}\njson: {}",
                            idx, e, &json[..json.len().min(200)]
                        );
                    }
                    _ => {}
                }

                unsafe {
                    if let Ok(slv) = sonic_rs::get_unchecked(json_bytes, [idx]) {
                        let _ = sonic_rs::from_str::<sonic_rs::Value>(slv.as_raw_str()).unwrap();
                    }
                }
            }
        }
    }

    // --- Strategy 3: Object iteration ---
    if jv.is_object() {
        let mut sonic_keys = Vec::new();
        for ret in sonic_rs::to_object_iter(json_bytes) {
            let (k, lv) = ret.unwrap();
            sonic_keys.push(k.to_string());
            // Verify the lazy value is parseable
            let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
        }

        // Unchecked variant
        unsafe {
            let mut unchecked_keys = Vec::new();
            for ret in sonic_rs::to_object_iter_unchecked(json_bytes) {
                let (k, lv) = ret.unwrap();
                unchecked_keys.push(k.to_string());
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            }
            assert_eq!(sonic_keys, unchecked_keys, "checked/unchecked key mismatch");
        }
    }

    // --- Strategy 4: Array iteration ---
    if jv.is_array() {
        let mut sonic_count = 0;
        for ret in sonic_rs::to_array_iter(json_bytes) {
            let lv = ret.unwrap();
            let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            sonic_count += 1;
        }
        assert_eq!(
            sonic_count,
            jv.as_array().unwrap().len(),
            "array iter count mismatch"
        );

        unsafe {
            let mut unchecked_count = 0;
            for ret in sonic_rs::to_array_iter_unchecked(json_bytes) {
                let lv = ret.unwrap();
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
                unchecked_count += 1;
            }
            assert_eq!(sonic_count, unchecked_count, "checked/unchecked count mismatch");
        }
    }

    // --- Strategy 5: Multi-level path (using keys collected from the value) ---
    let mut all_keys = Vec::new();
    input.value.collect_keys(&mut all_keys);
    if all_keys.len() >= 2 {
        // Try a 2-level path
        let path: Vec<&str> = all_keys.iter().take(2).map(|s| s.as_str()).collect();
        let _ = sonic_rs::get(json_bytes, &path);
        unsafe {
            let _ = sonic_rs::get_unchecked(json_bytes, &path);
        }
    }
});
