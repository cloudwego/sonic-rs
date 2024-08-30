#![no_main]
#![allow(clippy::mutable_key_type)]

use libfuzzer_sys::fuzz_target;
use serde_json::Value as JValue;
use sonic_rs::{
    from_slice, from_str, to_array_iter, to_array_iter_unchecked, to_object_iter,
    to_object_iter_unchecked, value::JsonContainerTrait, JsonNumberTrait, JsonValueTrait, Value,
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

fuzz_target!(|data: &[u8]| {
    match serde_json::from_slice::<JValue>(data) {
        Ok(jv) => {
            // compare from_slice result
            let sv: Value = from_slice(data).unwrap();
            compare_value(&jv, &sv);

            // compare to_string result
            let sout = sonic_rs::to_string(&sv).unwrap();
            let jv2 = serde_json::from_str::<JValue>(&sout).unwrap();
            let sv2: Value = from_str(&sout).unwrap();
            let eq = compare_value(&jv2, &sv2);

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

                    let gv = sonic_rs::get(data, [i]).unwrap();
                    compare_lazyvalue(jv, &gv);
                }

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
        Err(_) => {
            let _ = from_slice::<Value>(data).unwrap_err();
        }
    }

    test_type!(
        data, TestStruct, Enum, Foo, String, f64, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128
    );
});

fn compare_lazyvalue(jv: &JValue, sv: &sonic_rs::LazyValue) {
    let out = sv.as_raw_str().as_bytes();
    let sv2: sonic_rs::Value = sonic_rs::from_slice(out).unwrap();
    compare_value(jv, &sv2);
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

use std::{borrow::Cow, collections::HashMap, hash::Hash, marker::PhantomData};

use faststr::FastStr;
use serde::{Deserialize, Serialize};

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
