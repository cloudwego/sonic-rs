#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::Value as JValue;
use sonic_rs::dom_from_slice;
use sonic_rs::dom_from_str;
use sonic_rs::JsonNumberTrait;
use sonic_rs::JsonValue;
use sonic_rs::{to_array_iter, to_array_iter_unchecked, to_object_iter, to_object_iter_unchecked};

fuzz_target!(|data: &[u8]| {
    match serde_json::from_slice::<JValue>(data) {
        Ok(jv) => {
            let sv = dom_from_slice(data).unwrap();
            compare_value(&jv, sv.as_value());
            let sout = sonic_rs::to_string(&sv).unwrap();
            let jv2 = serde_json::from_str::<JValue>(&sout).unwrap();
            let sv2 = dom_from_str(&sout).unwrap();
            let eq = compare_value(&jv2, sv2.as_value());

            if jv.is_object() && eq {
                for ret in to_object_iter(data) {
                    let (k, lv) = ret.unwrap();
                    let jv = jv.get(k.as_str()).unwrap();
                    compare_lazyvalue(jv, &lv);

                    let gv = sonic_rs::get(data, &[k.as_str()]).unwrap();
                    compare_lazyvalue(jv, &gv);
                }

                // fuzzing unchecked apis
                unsafe {
                    for ret in to_object_iter_unchecked(data) {
                        let (k, lv) = ret.unwrap();
                        let jv = jv.get(k.as_str()).unwrap();
                        compare_lazyvalue(jv, &lv);

                        let gv = sonic_rs::get_unchecked(data, &[k.as_str()]).unwrap();
                        compare_lazyvalue(jv, &gv);
                    }
                }
            } else if jv.is_array() {
                for (i, ret) in to_array_iter(data).enumerate() {
                    let lv = ret.unwrap();
                    let jv = jv.get(i).unwrap();
                    compare_lazyvalue(jv, &lv);

                    let gv = sonic_rs::get(data, &[i]).unwrap();
                    compare_lazyvalue(jv, &gv);
                }

                // fuzzing unchecked apis
                unsafe {
                    for (i, ret) in to_array_iter_unchecked(data).enumerate() {
                        let lv = ret.unwrap();
                        let jv = jv.get(i).unwrap();
                        compare_lazyvalue(jv, &lv);

                        let gv = sonic_rs::get_unchecked(data, &[i]).unwrap();
                        compare_lazyvalue(jv, &gv);
                    }
                }
            }
        }
        Err(_) => {
            let _ = dom_from_slice(data).unwrap_err();
        }
    }
});

fn compare_lazyvalue(jv: &JValue, sv: &sonic_rs::LazyValue) {
    let out = sv.as_raw_slice();
    let sv2 = sonic_rs::dom_from_slice(out).unwrap();
    compare_value(jv, sv2.as_value());
}

fn compare_value(jv: &JValue, sv: &sonic_rs::Value) -> bool {
    match *jv {
        JValue::Object(ref obj) => {
            assert!(sv.is_object());
            let sobj = sv.as_object().unwrap();
            // because serde_json use a map to store object, and sonic_rs allows the repeated keys
            if sobj.len() == obj.len() {
                for (k, v) in obj {
                    let got = sobj.get(k).unwrap();
                    compare_value(v, got);
                }
                return true;
            } else {
                return false;
            }
        }
        JValue::Array(ref arr) => {
            assert!(sv.is_array());
            let sarr = sv.as_array().unwrap();
            assert!(arr.len() == sarr.len());

            for (i, v) in arr.iter().enumerate() {
                let got = sarr.get(i).unwrap();
                compare_value(v, got);
            }
        }
        JValue::Bool(b) => assert!(sv.is_boolean() && sv.as_bool().unwrap() == b),
        JValue::Null => assert!(sv.is_null()),
        JValue::Number(ref num) => {
            let got = sv.as_number().unwrap();
            if num.is_f64() {
                let jf = num.as_f64().unwrap();
                let sf = got.as_f64().unwrap();
                assert_eq!(jf, sf, "jf {} sf {}", jf, sf);
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
    true
}
