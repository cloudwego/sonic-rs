#![allow(clippy::mutable_key_type)]
pub mod gen;

use std::{borrow::Cow, collections::HashMap, hash::Hash, marker::PhantomData};

use faststr::FastStr;
use serde::{Deserialize, Serialize};
use serde_json::Value as JValue;
use sonic_rs::{
    from_slice, from_str, to_array_iter, to_array_iter_unchecked, to_object_iter,
    to_object_iter_unchecked, value::JsonContainerTrait, Deserializer, JsonNumberTrait,
    JsonValueTrait, LazyValue, OwnedLazyValue, Value,
};

macro_rules! test_type {
    ($data:expr, $($ty:ty),+) => {
        $(
            {
                match serde_json::from_slice::<$ty>($data) {
                    Ok(jv) => {
                        let sv: $ty = sonic_rs::from_slice::<$ty>($data).expect(&format!(
                            "parse valid json {:?} failed for type {}",
                            $data,
                            stringify!($ty)
                        ));
                        assert_eq!(sv, jv);

                        // Fuzz the struct to_string
                        let sout = sonic_rs::to_string(&sv).unwrap();
                        let jout = serde_json::to_string(&jv).unwrap();
                        let sv: $ty = sonic_rs::from_str::<$ty>(&sout).unwrap();
                        let jv: $ty = serde_json::from_str::<$ty>(&jout).unwrap();
                        assert_eq!(sv, jv);
                    }
                    Err(_) => {
                        let _ = sonic_rs::from_slice::<$ty>($data).expect_err(&format!(
                            "parse invalid json {:?} wrong for type {}",
                            $data,
                            stringify!($ty)
                        ));
                    }
                }
            }
        )*
    };
}

fn check_f32_literal_with<F>(literal: &str, parse: F) -> Result<(), String>
where
    F: Fn(&str) -> Result<f32, String>,
{
    match (literal.parse::<f32>(), parse(literal)) {
        (Ok(expected), Err(_)) if expected.is_infinite() => Ok(()),
        (Ok(expected), Ok(got)) if expected.is_infinite() => Err(format!(
            "sonic-rs accepted non-finite f32 literal {literal:?}: std={expected:e} (bits \
             {:#010x}), sonic={got:e} (bits {:#010x})",
            expected.to_bits(),
            got.to_bits(),
        )),
        (Ok(expected), Ok(got)) if expected.to_bits() == got.to_bits() => Ok(()),
        (Ok(expected), Ok(got)) => Err(format!(
            "f32 mismatch on {literal:?}: std={expected:e} (bits {:#010x}), sonic={got:e} (bits \
             {:#010x})",
            expected.to_bits(),
            got.to_bits(),
        )),
        (Ok(expected), Err(err)) => Err(format!(
            "sonic-rs rejected valid f32 literal {literal:?}: std={expected:e}, sonic error={err}"
        )),
        (Err(err), Ok(got)) => Err(format!(
            "sonic-rs accepted overflow/invalid f32 literal {literal:?}: std error={err}, \
             sonic={got:e} (bits {:#010x})",
            got.to_bits(),
        )),
        (Err(_), Err(_)) => Ok(()),
    }
}

fn assert_f32_literal_matches_std_parse(literal: &str) {
    check_f32_literal_with(literal, |literal| {
        sonic_rs::from_str::<f32>(literal).map_err(|err| err.to_string())
    })
    .unwrap_or_else(|err| panic!("{err}"));
}

pub fn fuzz_f32_literal_bytes(data: &[u8]) {
    if data.is_empty() || data.len() > 128 {
        return;
    }

    let Ok(literal) = std::str::from_utf8(data) else {
        return;
    };

    if literal.trim() != literal {
        return;
    }

    if !matches!(
        serde_json::from_str::<serde_json::Value>(literal),
        Ok(serde_json::Value::Number(_))
    ) {
        return;
    }

    assert_f32_literal_matches_std_parse(literal);
}

pub fn fuzz_number_input(input: &gen::NumberInput) {
    let literal = input.pattern.to_string();
    if !matches!(input.pattern, gen::NumberPattern::Raw(_)) {
        assert_f32_literal_matches_std_parse(&literal);
    }

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
                        got,
                        expected,
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
}

pub fn fuzz_string_value(input: &gen::JsonValue) {
    let json = input.to_json();
    let json_bytes = json.as_bytes();

    match serde_json::from_str::<serde_json::Value>(&json) {
        Ok(jv) => {
            let sv: sonic_rs::Value = sonic_rs::from_str(&json).unwrap_or_else(|e| {
                panic!(
                    "sonic-rs failed on valid JSON: {}\njson: {}",
                    e,
                    &json[..json.len().min(200)]
                )
            });

            compare_value(&jv, &sv);

            let out = sonic_rs::to_string(&sv).unwrap();
            let sv2: sonic_rs::Value = sonic_rs::from_str(&out).unwrap();
            let jv2: serde_json::Value = serde_json::from_str(&out).unwrap();
            compare_value(&jv2, &sv2);
        }
        Err(_) => {
            let _ = sonic_rs::from_str::<sonic_rs::Value>(&json);
        }
    }

    if let Ok(expected) = serde_json::from_str::<String>(&json) {
        let got: String = sonic_rs::from_str(&json).unwrap_or_else(|e| {
            panic!(
                "sonic-rs String deser failed: {}\njson: {}",
                e,
                &json[..json.len().min(200)]
            )
        });
        assert_eq!(
            got,
            expected,
            "String mismatch on: {}",
            &json[..json.len().min(200)]
        );
    }

    if sonic_rs::from_slice::<sonic_rs::Value>(json_bytes).is_ok() {
        let json_str = unsafe { std::str::from_utf8_unchecked(json_bytes) };
        let mut de = sonic_rs::Deserializer::from_str(json_str).utf8_lossy();
        let _: Result<sonic_rs::Value, _> = serde::Deserialize::deserialize(&mut de);
    }

    if let Ok(jv) = serde_json::from_str::<serde_json::Value>(&json) {
        if jv.is_string() {
            if let Ok(lv) = sonic_rs::from_str::<sonic_rs::LazyValue>(&json) {
                let raw = lv.as_raw_str();
                let sv: sonic_rs::Value = sonic_rs::from_str(raw).unwrap();
                assert!(sv.is_str());
                assert_eq!(sv.as_str().unwrap(), jv.as_str().unwrap());
            }
        }
    }
}

pub fn fuzz_get_path_value(input: &gen::JsonValue) {
    let json = input.to_json();
    let json_bytes = json.as_bytes();

    let jv: serde_json::Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(_) => return,
    };

    let _sv: sonic_rs::Value = match sonic_rs::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            panic!(
                "sonic-rs rejected valid JSON: {}\njson: {}",
                e,
                &json[..json.len().min(200)]
            )
        }
    };

    if let Some(obj) = jv.as_object() {
        for (key, expected) in obj.iter().take(8) {
            match sonic_rs::get(json_bytes, &[key.as_str()]) {
                Ok(slv) => {
                    let reparsed: sonic_rs::Value = sonic_rs::from_str(slv.as_raw_str()).unwrap();
                    compare_value(expected, &reparsed);
                }
                Err(e) => {
                    panic!(
                        "sonic-rs get() failed but serde_json found key {:?}: {}\njson: {}",
                        key,
                        e,
                        &json[..json.len().min(200)]
                    );
                }
            }

            unsafe {
                if let Ok(slv) = sonic_rs::get_unchecked(json_bytes, &[key.as_str()]) {
                    let reparsed: sonic_rs::Value = sonic_rs::from_str(slv.as_raw_str()).unwrap();
                    compare_value(expected, &reparsed);
                }
            }

            if let Some(child_obj) = expected.as_object() {
                for (child_key, child_expected) in child_obj.iter().take(4) {
                    match sonic_rs::get(json_bytes, &[key.as_str(), child_key.as_str()]) {
                        Ok(slv) => {
                            let reparsed: sonic_rs::Value =
                                sonic_rs::from_str(slv.as_raw_str()).unwrap();
                            compare_value(child_expected, &reparsed);
                        }
                        Err(e) => {
                            panic!(
                                "sonic-rs get() failed on nested path {:?}/{:?}: {}\njson: {}",
                                key,
                                child_key,
                                e,
                                &json[..json.len().min(200)]
                            );
                        }
                    }
                }
            }
        }

        let mut sonic_keys = Vec::new();
        for ret in sonic_rs::to_object_iter(json_bytes) {
            let (k, lv) = ret.unwrap();
            sonic_keys.push(k.to_string());
            let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
        }

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

    if let Some(arr) = jv.as_array() {
        for (idx, expected) in arr.iter().enumerate().take(8) {
            match sonic_rs::get(json_bytes, [idx]) {
                Ok(slv) => {
                    let reparsed: sonic_rs::Value = sonic_rs::from_str(slv.as_raw_str()).unwrap();
                    compare_value(expected, &reparsed);
                }
                Err(e) => {
                    panic!(
                        "sonic-rs get([{}]) failed but serde_json found: {}\njson: {}",
                        idx,
                        e,
                        &json[..json.len().min(200)]
                    );
                }
            }

            unsafe {
                if let Ok(slv) = sonic_rs::get_unchecked(json_bytes, [idx]) {
                    let reparsed: sonic_rs::Value = sonic_rs::from_str(slv.as_raw_str()).unwrap();
                    compare_value(expected, &reparsed);
                }
            }
        }

        let mut sonic_count = 0;
        for ret in sonic_rs::to_array_iter(json_bytes) {
            let lv = ret.unwrap();
            let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            sonic_count += 1;
        }
        assert_eq!(sonic_count, arr.len(), "array iter count mismatch");

        unsafe {
            let mut unchecked_count = 0;
            for ret in sonic_rs::to_array_iter_unchecked(json_bytes) {
                let lv = ret.unwrap();
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
                unchecked_count += 1;
            }
            assert_eq!(
                sonic_count, unchecked_count,
                "checked/unchecked count mismatch"
            );
        }
    }
}

pub fn fuzz_deep_nesting_input(input: &gen::DeepNestInput) {
    let json = input.to_json();
    let json_bytes = json.as_bytes();

    let sv_result = sonic_rs::from_str::<sonic_rs::Value>(&json);
    let jv_result = serde_json::from_str::<serde_json::Value>(&json);

    match (&jv_result, &sv_result) {
        (Ok(jv), Ok(sv)) => {
            compare_value(jv, sv);

            let out = sonic_rs::to_string(sv).unwrap();
            let sv2: sonic_rs::Value = sonic_rs::from_str(&out).unwrap();
            let jv2: serde_json::Value = serde_json::from_str(&out).unwrap();
            compare_value(&jv2, &sv2);
        }
        (Err(_), Err(_)) => {}
        (Ok(_), Err(e)) => {
            panic!(
                "sonic-rs rejected valid deep JSON: {}\njson len: {}",
                e,
                json.len()
            );
        }
        (Err(_), Ok(_)) => {}
    }

    if jv_result.is_ok() {
        if let Ok(olv) = sonic_rs::from_str::<sonic_rs::OwnedLazyValue>(&json) {
            walk_owned_lazy_seed(&olv, 0);
        }
    }

    if let Ok(jv) = &jv_result {
        if jv.is_array() {
            for ret in sonic_rs::to_array_iter(json_bytes) {
                let lv = ret.unwrap();
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            }
        } else if jv.is_object() {
            for ret in sonic_rs::to_object_iter(json_bytes) {
                let (_, lv) = ret.unwrap();
                let _ = sonic_rs::from_str::<sonic_rs::Value>(lv.as_raw_str()).unwrap();
            }
        }
    }

    if jv_result.is_ok() {
        let sv: sonic_rs::Value = sonic_rs::from_slice(json_bytes).unwrap();
        let out = sonic_rs::to_string(&sv).unwrap();
        let _ = sonic_rs::from_str::<sonic_rs::Value>(&out).unwrap();
    }
}

pub fn fuzz_serde_roundtrip_raw(data: &[u8]) {
    macro_rules! try_type {
        ($ty:ty) => {
            match serde_json::from_slice::<$ty>(data) {
                Ok(expected) => {
                    if let Ok(got) = sonic_rs::from_slice::<$ty>(data) {
                        assert_eq!(
                            got,
                            expected,
                            "type {} mismatch on raw input",
                            stringify!($ty)
                        );
                    }
                }
                Err(_) => {
                    let _ = sonic_rs::from_slice::<$ty>(data);
                }
            }
        };
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct SimpleStruct {
        name: String,
        value: i64,
        active: bool,
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    enum TestEnum {
        Unit,
        Newtype(i32),
        Tuple(String, i64),
        Struct { a: bool, b: String },
    }

    try_type!(SimpleStruct);
    try_type!(TestEnum);
    try_type!(Vec<i64>);
    try_type!(HashMap<String, String>);
    try_type!(Option<String>);
    try_type!(String);
    try_type!(f64);
    try_type!(bool);
}

pub fn sonic_rs_fuzz_data(data: &[u8]) {
    match serde_json::from_slice::<JValue>(data) {
        Ok(jv) => {
            // compare from_slice result
            let sv: Value = from_slice(data).unwrap();
            let eq = compare_value(&jv, &sv);

            // compare to_string result
            let sout = sonic_rs::to_string(&sv).unwrap();
            let jv2 = serde_json::from_str::<JValue>(&sout).unwrap();
            let sv2: Value = from_str(&sout).unwrap();
            compare_value(&jv2, &sv2);

            fuzz_utf8_lossy(data, &sv);

            if jv.is_object() && eq {
                let owned: OwnedLazyValue = sonic_rs::from_slice(data).unwrap();
                for ret in to_object_iter(data) {
                    let (k, lv) = ret.unwrap();
                    let jv = jv.get(k.as_ref()).unwrap();
                    let ov = owned.get(k.as_ref()).unwrap();
                    compare_owned_lazyvalue(jv, ov);
                    compare_lazyvalue(jv, &lv);

                    let gv = sonic_rs::get(data, &[k.as_ref()]).unwrap();
                    compare_lazyvalue(jv, &gv);
                }
                compare_owned_lazyvalue(&jv, &owned);

                // fuzzing unchecked apis
                unsafe {
                    for ret in to_object_iter_unchecked(data) {
                        let (k, lv) = ret.unwrap();
                        let jv = jv.get(k.as_ref()).unwrap();
                        compare_lazyvalue(jv, &lv);

                        let gv = sonic_rs::get_unchecked(data, &[k.as_ref()]).unwrap();
                        compare_lazyvalue(jv, &gv);
                    }
                }
            } else if jv.is_array() && eq {
                let owned: OwnedLazyValue = sonic_rs::from_slice(data).unwrap();
                for (i, ret) in to_array_iter(data).enumerate() {
                    let lv = ret.unwrap();
                    let jv = jv.get(i).unwrap();
                    compare_lazyvalue(jv, &lv);
                    let ov = owned.get(i).unwrap();
                    compare_owned_lazyvalue(jv, ov);

                    let gv = sonic_rs::get(data, [i]).unwrap();
                    compare_lazyvalue(jv, &gv);
                }
                compare_owned_lazyvalue(&jv, &owned);

                // fuzzing unchecked apis
                unsafe {
                    for (i, ret) in to_array_iter_unchecked(data).enumerate() {
                        let lv = ret.unwrap();
                        let jv = jv.get(i).unwrap();
                        compare_lazyvalue(jv, &lv);

                        let gv = sonic_rs::get_unchecked(data, [i]).unwrap();
                        compare_lazyvalue(jv, &gv);
                    }
                }
            }
        }
        Err(e) => {
            let _ = from_slice::<Value>(data).expect_err(&format!(
                "parse invalid json {:?} failed, should return error {e} ",
                data
            ));

            // LazyValue should return error if the json is invalid
            let msg = e.to_string();
            if (msg.starts_with("expected ") || msg.starts_with("EOF"))
                && simdutf8::basic::from_utf8(data).is_ok()
            {
                let _ = from_slice::<OwnedLazyValue>(data).expect_err(&format!(
                    "parse invalid json {:?} failed, should return error {msg}",
                    data
                ));
            }
        }
    }

    test_type!(
        data, TestStruct, Enum, Foo, String, f64, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128
    );
}

fn compare_lazyvalue(jv: &JValue, sv: &LazyValue) {
    let out = sv.as_raw_str().as_bytes();
    let sv2: sonic_rs::Value = sonic_rs::from_slice(out).unwrap();
    compare_value(jv, &sv2);
}

fn compare_owned_lazyvalue(jv: &JValue, sv: &OwnedLazyValue) {
    match *jv {
        JValue::Object(ref obj) => {
            assert!(sv.is_object());
            for (k, v) in obj {
                let got = sv.get(k).unwrap();
                compare_owned_lazyvalue(v, got);
            }
        }
        JValue::Array(ref arr) => {
            assert!(sv.is_array());
            for (i, v) in arr.iter().enumerate() {
                let got = sv.get(i).unwrap();
                compare_owned_lazyvalue(v, got);
            }
        }
        JValue::Bool(b) => assert!(sv.is_boolean() && sv.as_bool().unwrap() == b),
        JValue::Null => assert!(sv.is_null()),
        JValue::Number(ref num) => {
            let got = sv.as_number().unwrap();
            if num.is_f64() {
                assert_eq!(num.as_f64(), got.as_f64());
            }
            if num.is_u64() {
                assert_eq!(num.as_u64(), got.as_u64());
            }
            if num.is_i64() {
                assert_eq!(num.as_i64(), got.as_i64());
            }
        }
        JValue::String(ref s) => {
            assert!(sv.is_str());
            assert_eq!(sv.as_str().unwrap(), s);
        }
    }
}

fn fuzz_utf8_lossy(json: &[u8], sv: &sonic_rs::Value) {
    let json = unsafe { std::str::from_utf8_unchecked(json) };
    let mut de = Deserializer::from_str(json).utf8_lossy();
    let value: Value = Deserialize::deserialize(&mut de).unwrap();
    let out = sonic_rs::to_string(&value).unwrap();
    let got: Value = sonic_rs::from_str(&out).unwrap();
    assert_eq!(&got, sv);
}

fn walk_owned_lazy_seed(v: &sonic_rs::OwnedLazyValue, depth: usize) {
    if depth > 600 {
        return;
    }

    if v.is_object() {
        if let Some(child) = v.get("k") {
            walk_owned_lazy_seed(child, depth + 1);
        }
    } else if v.is_array() {
        for i in 0..16 {
            match v.get(i) {
                Some(child) => walk_owned_lazy_seed(child, depth + 1),
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

pub fn compare_value(jv: &JValue, sv: &sonic_rs::Value) -> bool {
    match *jv {
        JValue::Object(ref obj) => {
            assert!(sv.is_object());
            let sobj = sv.as_object().unwrap();
            // because serde_json use a map to store object, and sonic_rs allows the repeated keys
            if sobj.len() == obj.len() {
                for (k, v) in obj {
                    let got = sobj.get(k).unwrap();
                    if !compare_value(v, got) {
                        return false;
                    }
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
                if !compare_value(v, got) {
                    return false;
                }
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
    };
    true
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Foo {
    name: FastStr,
    id: u64,
}

#[derive(Debug, Deserialize, Serialize, Hash, Eq, PartialEq)]
enum Enum {
    Zero = 0,
    One = 1,
    Two = 2,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
enum FieldEnum {
    Integer(i8),
    Tuple((FastStr, i32)),
    Struct(Foo),
    Unit,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
enum FieldlessEnum {
    Tuple(),
    Struct {},
    Unit,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Wrapper<'a>(&'a str);

// A unit struct
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Unit;

// A uint struct
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Phan<T> {
    phan: String,
    _data: PhantomData<T>,
}

// A tuple struct
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Pair(i32, f32);

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct TestStruct<'a> {
    fieldless: FieldlessEnum,
    enummap: HashMap<Enum, FieldlessEnum>,
    enum_: Enum,

    // basic types
    boolean: bool,
    integer: i32,
    float: f64,
    int128: i128,
    uint128: u128,
    char_: char,

    // string or bytes
    str_: &'a str,
    // bytes_: &'a [u8],
    string: String,
    faststr: FastStr,
    #[serde(borrow)]
    cow: Cow<'a, str>,

    // containers
    vector: Vec<u32>,
    array: [u32; 1],
    empty_array: [u8; 0],
    map: HashMap<FastStr, f64>,
    map_opkey: HashMap<Option<FastStr>, f64>,

    // enum types
    option: Option<String>,
    fieldenum: FieldEnum,

    // tuple or struct
    tuple: (u64, String),
    tuple_struct: Pair,
    unit_struct: Unit,

    #[serde(borrow)]
    wrapper: Wrapper<'a>,
    phan_struct: Phan<()>,
}

#[cfg(test)]
mod test {
    use crate::*;

    fn ascii(s: &str) -> gen::JsonString {
        gen::JsonString::Ascii(gen::AsciiStr(s.to_owned()))
    }

    fn escaped(s: &str) -> gen::JsonString {
        gen::JsonString::WithEscapes(gen::EscapeStr(s.to_owned()))
    }

    fn unicode(s: &str) -> gen::JsonString {
        gen::JsonString::Unicode(gen::UnicodeStr(s.to_owned()))
    }

    fn test_compare_value(data: &[u8]) -> bool {
        let sv = sonic_rs::from_slice(data).unwrap();
        let jv = serde_json::from_slice(data).unwrap();
        compare_value(&jv, &sv)
    }

    #[test]
    fn test_case() {
        sonic_rs_fuzz_data(br#"[[{"1":4,        "":80}          ]]"#);
        assert!(test_compare_value(
            br#"[[{"1":4,        "":80}          ]]"#
        ));
        assert!(!test_compare_value(
            br#"[[{"":4,        "":80}          ]]"#
        ));

        sonic_rs_fuzz_data(br#"[45, 48, 10]"#);
    }

    #[test]
    fn test_f32_literal_alignment_on_boundaries() {
        for literal in [
            "100e11",
            "17005001.000000000000130",
            "3.4028235e38",
            "3.4028236e38",
            "1e39",
        ] {
            assert_f32_literal_matches_std_parse(literal);
        }
    }

    #[test]
    fn test_fuzz_number_input_f32_edges() {
        for input in [
            gen::NumberInput {
                pattern: gen::NumberPattern::Edge(gen::NumberEdge::F32DisguisedFastPath),
                in_array: false,
            },
            gen::NumberInput {
                pattern: gen::NumberPattern::Edge(gen::NumberEdge::F32TieBoundary),
                in_array: false,
            },
            gen::NumberInput {
                pattern: gen::NumberPattern::Edge(gen::NumberEdge::F32MaxFinite),
                in_array: false,
            },
            gen::NumberInput {
                pattern: gen::NumberPattern::Edge(gen::NumberEdge::F32Overflow),
                in_array: false,
            },
            gen::NumberInput {
                pattern: gen::NumberPattern::Edge(gen::NumberEdge::F32DisguisedFastPath),
                in_array: true,
            },
        ] {
            fuzz_number_input(&input);
        }
    }

    #[test]
    fn test_f32_fuzz_helper_catches_mocked_boundary_regression() {
        let err = check_f32_literal_with("100e11", |literal| {
            if literal == "100e11" {
                Ok(1e11_f32)
            } else {
                literal.parse::<f32>().map_err(|e| e.to_string())
            }
        })
        .unwrap_err();

        assert!(err.contains("100e11"));
        assert!(err.contains("f32 mismatch"));
    }

    #[test]
    fn test_fuzz_f32_literal_bytes_seeds() {
        for seed in [
            b"100e11".as_slice(),
            b"17005001.000000000000130".as_slice(),
            b"3.4028235e38".as_slice(),
            b"3.4028236e38".as_slice(),
            b"1e39".as_slice(),
        ] {
            fuzz_f32_literal_bytes(seed);
        }
    }

    #[test]
    fn test_sonic_rs_fuzz_data_seed_suite() {
        for seed in [
            br#"{"k":1,"k":2,"nested":{"k":3,"arr":[{"k":4},5,{"inner":[1,{"k":"leaf"}]}]}}"#
                .as_slice(),
            br#"{"text":"line\n\tslash\/quote\"backslash\\","unicode":"\u4E2D\u6587","emoji":"\uD83D\uDE00"}"#
                .as_slice(),
            br#"[0,-0,18446744073709551615,9007199254740993,0.00000000000000001,100e11,17005001.000000000000130,3.4028235e38,3.4028236e38]"#
                .as_slice(),
        ] {
            sonic_rs_fuzz_data(seed);
        }
    }

    #[test]
    fn test_fuzz_string_value_seeds() {
        for value in [
            gen::JsonValue::Str(ascii("plain ascii")),
            gen::JsonValue::Str(escaped(r#"\n\t\\\/\""#)),
            gen::JsonValue::Str(unicode("中😀é")),
            gen::JsonValue::Object(vec![
                (
                    ascii("text"),
                    gen::JsonValue::Str(escaped(r#"\u0000\u001F\uD83D\uDE00"#)),
                ),
                (
                    ascii("arr"),
                    gen::JsonValue::Array(vec![
                        gen::JsonValue::Str(ascii("leaf")),
                        gen::JsonValue::Str(escaped(r#"\b\f\r"#)),
                    ]),
                ),
            ]),
        ] {
            fuzz_string_value(&value);
        }
    }

    #[test]
    fn test_fuzz_get_path_value_seeds() {
        for value in [
            gen::JsonValue::Object(vec![
                (ascii("alpha"), gen::JsonValue::Int(1)),
                (
                    ascii("beta"),
                    gen::JsonValue::Object(vec![(
                        ascii("gamma"),
                        gen::JsonValue::Object(vec![(
                            ascii("delta"),
                            gen::JsonValue::Str(ascii("leaf")),
                        )]),
                    )]),
                ),
                (
                    ascii("root_arr"),
                    gen::JsonValue::Array(vec![
                        gen::JsonValue::Bool(true),
                        gen::JsonValue::Object(vec![(
                            ascii("name"),
                            gen::JsonValue::Str(ascii("node")),
                        )]),
                    ]),
                ),
            ]),
            gen::JsonValue::Array(vec![
                gen::JsonValue::Object(vec![(
                    ascii("k"),
                    gen::JsonValue::Array(vec![
                        gen::JsonValue::Uint(0),
                        gen::JsonValue::Object(vec![(
                            ascii("z"),
                            gen::JsonValue::Str(ascii("tail")),
                        )]),
                    ]),
                )]),
                gen::JsonValue::Array(vec![
                    gen::JsonValue::Bool(false),
                    gen::JsonValue::Null,
                    gen::JsonValue::Object(vec![(
                        ascii("more"),
                        gen::JsonValue::Str(ascii("paths")),
                    )]),
                ]),
            ]),
        ] {
            fuzz_get_path_value(&value);
        }
    }

    #[test]
    fn test_fuzz_deep_nesting_input_seeds() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                for input in [
                    gen::DeepNestInput {
                        pattern: gen::NestPattern::DeepArray { depth: 32 },
                    },
                    gen::DeepNestInput {
                        pattern: gen::NestPattern::DeepObject { depth: 32 },
                    },
                    gen::DeepNestInput {
                        pattern: gen::NestPattern::WideArray { count: 64 },
                    },
                    gen::DeepNestInput {
                        pattern: gen::NestPattern::WideObject { count: 64 },
                    },
                    gen::DeepNestInput {
                        pattern: gen::NestPattern::Mixed { depth: 4, width: 3 },
                    },
                ] {
                    fuzz_deep_nesting_input(&input);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn test_fuzz_serde_roundtrip_raw_seeds() {
        for seed in [
            br#"{"name":"alice","value":42,"active":true}"#.as_slice(),
            br#""hello\nworld""#.as_slice(),
            br#"true"#.as_slice(),
            br#"[1,2,3,4]"#.as_slice(),
            br#"{"x":"1","y":"2"}"#.as_slice(),
            br#""Unit""#.as_slice(),
            br#"{"Newtype":123}"#.as_slice(),
            br#"{"Tuple":["hello",-7]}"#.as_slice(),
            br#"{"Struct":{"a":true,"b":"ok"}}"#.as_slice(),
        ] {
            fuzz_serde_roundtrip_raw(seed);
        }
    }
}
