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
}
