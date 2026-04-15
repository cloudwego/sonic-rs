//! Fuzz target for serde round-trip — exercises serialization/deserialization
//! of complex Rust types through sonic-rs, comparing against serde_json.
#![no_main]

use std::collections::HashMap;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, Serialize};

/// Complex struct that exercises many serde features.
#[derive(Debug, Serialize, Deserialize, PartialEq, Arbitrary)]
struct ComplexStruct {
    // Primitives
    bool_val: bool,
    u8_val: u8,
    u16_val: u16,
    u32_val: u32,
    u64_val: u64,
    i8_val: i8,
    i16_val: i16,
    i32_val: i32,
    i64_val: i64,

    // String types
    string_val: String,

    // Containers
    vec_u32: Vec<u32>,
    vec_string: Vec<String>,
    map_val: HashMap<String, i64>,
    option_some: Option<String>,
    option_none: Option<u64>,

    // Nested
    nested: NestedStruct,
    enum_val: TestEnum,
    vec_enum: Vec<TestEnum>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Arbitrary)]
struct NestedStruct {
    x: FiniteF64,
    y: FiniteF64,
    label: String,
    tags: Vec<String>,
}

/// A finite f64 that can be serialized to JSON (no NaN/Inf).
#[derive(Debug, PartialEq)]
struct FiniteF64(f64);

impl<'a> Arbitrary<'a> for FiniteF64 {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let f: f64 = u.arbitrary()?;
        Ok(FiniteF64(if f.is_finite() { f } else { 0.0 }))
    }
}

impl Serialize for FiniteF64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for FiniteF64 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let f = f64::deserialize(deserializer)?;
        Ok(FiniteF64(f))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Arbitrary)]
enum TestEnum {
    Unit,
    Newtype(i32),
    Tuple(String, i64),
    Struct { a: bool, b: String },
}

/// Simpler struct for broader coverage with less complexity.
#[derive(Debug, Serialize, Deserialize, PartialEq, Arbitrary)]
struct SimpleStruct {
    name: String,
    value: i64,
    active: bool,
}

/// Struct with optional/nullable fields.
#[derive(Debug, Serialize, Deserialize, PartialEq, Arbitrary)]
struct OptionalFields {
    required: String,
    opt_str: Option<String>,
    opt_num: Option<i64>,
    opt_bool: Option<bool>,
    opt_vec: Option<Vec<i32>>,
    opt_nested: Option<SimpleStruct>,
}

/// Enum with all variant kinds.
#[derive(Debug, Serialize, Deserialize, PartialEq, Arbitrary)]
enum FullEnum {
    Unit,
    Newtype(String),
    Tuple(i32, bool, String),
    Struct {
        id: u64,
        name: String,
        data: Vec<u8>,
    },
}

/// Wrapper for testing with structured JSON input.
#[derive(Debug, Arbitrary)]
enum FuzzInput {
    Complex(ComplexStruct),
    Simple(SimpleStruct),
    Optional(OptionalFields),
    Enum(FullEnum),
    VecComplex(Vec<SimpleStruct>),
    MapComplex(HashMap<String, SimpleStruct>),
    /// Raw JSON bytes for unstructured testing.
    Raw(Vec<u8>),
}

fuzz_target!(|input: FuzzInput| {
    match input {
        FuzzInput::Complex(v) => roundtrip_test(&v),
        FuzzInput::Simple(v) => roundtrip_test(&v),
        FuzzInput::Optional(v) => roundtrip_test(&v),
        FuzzInput::Enum(v) => roundtrip_test(&v),
        FuzzInput::VecComplex(v) => roundtrip_test(&v),
        FuzzInput::MapComplex(v) => roundtrip_test(&v),
        FuzzInput::Raw(data) => sonic_rs_fuzz::fuzz_serde_roundtrip_raw(&data),
    }
});

/// Serialize with sonic-rs and serde_json, then cross-deserialize and compare.
fn roundtrip_test<T>(value: &T)
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    // Filter out NaN/Inf which aren't valid JSON
    let sonic_json = match sonic_rs::to_string(value) {
        Ok(s) => s,
        Err(_) => return, // Can't serialize (e.g., NaN float) — skip
    };
    let serde_json_str = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Both serializations should produce parseable JSON
    let sonic_parsed: T = sonic_rs::from_str(&sonic_json).unwrap_or_else(|e| {
        panic!(
            "sonic-rs can't parse its own output: {}\njson: {}",
            e,
            &sonic_json[..sonic_json.len().min(500)]
        )
    });
    let serde_parsed: T = serde_json::from_str(&serde_json_str).unwrap();

    // Self-roundtrip should be identity
    assert_eq!(
        &sonic_parsed,
        value,
        "sonic-rs roundtrip changed value\njson: {}",
        &sonic_json[..sonic_json.len().min(500)]
    );
    assert_eq!(&serde_parsed, value, "serde_json roundtrip changed value");

    // Cross-deserialize: parse sonic-rs output with serde_json and vice versa
    let cross1: T = serde_json::from_str(&sonic_json).unwrap_or_else(|e| {
        panic!(
            "serde_json can't parse sonic-rs output: {}\njson: {}",
            e,
            &sonic_json[..sonic_json.len().min(500)]
        )
    });
    let cross2: T = sonic_rs::from_str(&serde_json_str).unwrap_or_else(|e| {
        panic!(
            "sonic-rs can't parse serde_json output: {}\njson: {}",
            e,
            &serde_json_str[..serde_json_str.len().min(500)]
        )
    });

    assert_eq!(&cross1, value, "cross-deser (serde reads sonic) mismatch");
    assert_eq!(&cross2, value, "cross-deser (sonic reads serde) mismatch");

    // Value-level comparison
    let sv: sonic_rs::Value = sonic_rs::from_str(&sonic_json).unwrap();
    let jv: serde_json::Value = serde_json::from_str(&sonic_json).unwrap();
    sonic_rs_fuzz::compare_value(&jv, &sv);
}
