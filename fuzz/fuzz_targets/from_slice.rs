#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::Value as JValue;
use sonic_rs::dom_from_slice;
use sonic_rs::dom_from_str;
use sonic_rs::JsonNumberTrait;
use sonic_rs::JsonValue;

fuzz_target!(|data: &[u8]| {
    match serde_json::from_slice::<JValue>(data) {
        Ok(jv) => {
            let sv = dom_from_slice(data).unwrap();
            compare(&jv, sv.as_value());
            let sout = sonic_rs::to_string(&sv).unwrap();
            let jv2 = serde_json::from_str::<JValue>(&sout).unwrap();
            let sv2 = dom_from_str(&sout).unwrap();
            compare(&jv2, sv2.as_value());
        }
        Err(_) => {
            let _ = dom_from_slice(data).unwrap_err();
            // assert!(err.is_syntax());
        }
    }
});

fn compare(jv: &JValue, sv: &sonic_rs::Value) {
    match *jv {
        JValue::Object(ref obj) => {
            assert!(sv.is_object());
            let sobj = sv.as_object().unwrap();
            // because serde_json use a map to store object, and sonic_rs allows the repeated keys
            if sobj.len() == obj.len() {
                for (k, v) in obj {
                    let got = sobj.get(k).unwrap();
                    compare(v, got)
                }
            }
        }
        JValue::Array(ref arr) => {
            assert!(sv.is_array());
            let sarr = sv.as_array().unwrap();
            assert!(arr.len() == sarr.len());

            for (i, v) in arr.iter().enumerate() {
                let got = sarr.get(i).unwrap();
                compare(v, got)
            }
        }
        JValue::Bool(b) => assert!(sv.is_boolean() && sv.as_bool().unwrap() == b),
        JValue::Null => assert!(sv.is_null()),
        JValue::Number(ref num) => {
            let got = sv.as_number().unwrap();
            if num.is_f64() {
                assert!(num.as_f64().unwrap() == got.as_f64().unwrap());
            }
            if num.is_u64() {
                assert!(num.as_u64().unwrap() == got.as_u64().unwrap());
            }
            if num.is_i64() {
                assert!(num.as_i64().unwrap() == got.as_i64().unwrap());
            }
        }
        JValue::String(ref s) => assert!(sv.is_str() && sv.as_str().unwrap() == s),
    }
}
