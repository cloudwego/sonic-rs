#![cfg(feature = "derive")]

use std::{fmt::Write, hint::black_box, time::Instant};

use serde::{Deserialize, Deserializer, Serialize};
use sonic_rs::SonicDeserialize;

fn default_tag() -> i64 {
    77
}

fn number_or_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Number(i64),
        String(String),
    }

    match Repr::deserialize(deserializer)? {
        Repr::Number(value) => Ok(value),
        Repr::String(value) => value.parse().map_err(serde::de::Error::custom),
    }
}

macro_rules! define_wide {
    ($name:ident, $deserialize:path) => {
        #[derive(Debug, Default, PartialEq, Serialize, $deserialize)]
        #[serde(default)]
        struct $name {
            #[serde(alias = "uid")]
            user_id: i64,
            #[serde(default, deserialize_with = "number_or_string")]
            coerced: i64,
            #[serde(default = "default_tag")]
            tag: i64,
            #[serde(skip_deserializing)]
            skipped: i64,
            #[serde(rename(deserialize = "wire_name", serialize = "wire_out"), default)]
            renamed: i64,
            f01: i64,
            f02: i64,
            f03: i64,
            f04: i64,
            f05: i64,
            f06: i64,
            f07: i64,
            f08: i64,
            f09: i64,
            f10: i64,
            f11: i64,
            f12: i64,
            f13: i64,
            f14: i64,
            f15: i64,
            f16: i64,
            f17: i64,
            f18: i64,
            f19: i64,
            f20: i64,
            f21: i64,
            f22: i64,
            f23: i64,
            f24: i64,
            f25: i64,
            f26: i64,
            f27: i64,
            f28: i64,
            f29: i64,
            f30: i64,
            f31: i64,
            f32: i64,
            f33: i64,
            f34: i64,
            f35: i64,
            f36: i64,
            f37: i64,
            f38: i64,
            f39: i64,
            f40: i64,
            f41: i64,
            f42: i64,
            f43: i64,
            f44: i64,
            f45: i64,
            f46: i64,
            f47: i64,
            f48: i64,
            f49: i64,
            f50: i64,
            f51: i64,
            f52: i64,
            f53: i64,
            f54: i64,
            f55: i64,
            f56: i64,
            f57: i64,
            f58: i64,
            f59: i64,
            f60: i64,
            f61: i64,
            f62: i64,
            f63: i64,
            f64: i64,
            f65: i64,
            f66: i64,
            f67: i64,
            f68: i64,
            f69: i64,
            f70: i64,
            f71: i64,
            f72: i64,
            f73: i64,
            f74: i64,
            f75: i64,
            f76: i64,
            f77: i64,
            f78: i64,
            f79: i64,
            f80: i64,
            f81: i64,
            f82: i64,
            f83: i64,
            f84: i64,
            f85: i64,
            f86: i64,
            f87: i64,
            f88: i64,
            f89: i64,
            f90: i64,
            f91: i64,
            f92: i64,
            f93: i64,
            f94: i64,
            f95: i64,
            f96: i64,
            f97: i64,
            f98: i64,
            f99: i64,
            f100: i64,
            f101: i64,
            f102: i64,
            f103: i64,
            f104: i64,
            f105: i64,
            f106: i64,
            f107: i64,
            f108: i64,
            f109: i64,
            f110: i64,
            f111: i64,
            f112: i64,
            f113: i64,
            f114: i64,
            f115: i64,
            f116: i64,
            f117: i64,
            f118: i64,
            f119: i64,
            f120: i64,
            f121: i64,
            f122: i64,
            f123: i64,
            f124: i64,
            f125: i64,
            f126: i64,
            f127: i64,
        }
    };
}

define_wide!(ControlWide, Deserialize);
define_wide!(FastWide, SonicDeserialize);

fn wide_payload() -> String {
    let mut json = String::from(r#"{"uid":7,"coerced":"19","wire_name":8"#);
    for index in 1..=127 {
        write!(&mut json, ",\"f{index:02}\":{index}").unwrap();
    }
    json.push('}');
    json
}

fn assert_same(control: &ControlWide, fast: &FastWide) {
    let control = serde_json::to_value(control).unwrap();
    let fast = serde_json::to_value(fast).unwrap();
    assert_eq!(fast, control);
}

#[test]
fn phf_path_matches_serde_for_alias_rename_default_skip_and_custom_decoder() {
    let json = r#"{
        "uid": 42,
        "coerced": "19",
        "wire_name": 8,
        "skipped": 999,
        "f01": 1,
        "f31": 31,
        "unknown": {"ignored": true}
    }"#;

    let control: ControlWide = sonic_rs::from_str(json).unwrap();
    let fast: FastWide = sonic_rs::from_str(json).unwrap();
    assert_same(&control, &fast);
    assert_eq!(fast.user_id, 42);
    assert_eq!(fast.coerced, 19);
    assert_eq!(fast.tag, 77);
    assert_eq!(fast.skipped, 0);
    assert_eq!(fast.renamed, 8);

    let via_serde_json: FastWide = serde_json::from_str(json).unwrap();
    assert_eq!(via_serde_json, fast);
}

#[test]
fn canonical_plus_alias_is_a_duplicate() {
    let json = r#"{"user_id":1,"uid":2}"#;
    let control = sonic_rs::from_str::<ControlWide>(json)
        .unwrap_err()
        .to_string();
    let fast = sonic_rs::from_str::<FastWide>(json)
        .unwrap_err()
        .to_string();
    assert!(control.contains("duplicate field"), "{control}");
    assert!(fast.contains("duplicate field"), "{fast}");
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlStrict {
    id: i64,
}

#[derive(Debug, PartialEq, SonicDeserialize)]
#[serde(deny_unknown_fields)]
#[sonic(force_phf)]
struct FastStrict {
    id: i64,
}

#[test]
fn deny_unknown_fields_matches_serde() {
    let json = r#"{"id":1,"extra":2}"#;
    let control = sonic_rs::from_str::<ControlStrict>(json)
        .unwrap_err()
        .to_string();
    let fast = sonic_rs::from_str::<FastStrict>(json)
        .unwrap_err()
        .to_string();
    assert!(control.contains("unknown field"), "{control}");
    assert!(fast.contains("unknown field"), "{fast}");
}

#[derive(Debug, PartialEq, Deserialize)]
struct ControlSmall {
    #[serde(alias = "identifier")]
    id: i64,
}

#[derive(Debug, PartialEq, SonicDeserialize)]
struct FastSmall {
    #[serde(alias = "identifier")]
    id: i64,
}

#[test]
fn small_struct_uses_the_direct_match_fallback() {
    let control: ControlSmall = sonic_rs::from_str(r#"{"identifier":3}"#).unwrap();
    let fast: FastSmall = sonic_rs::from_str(r#"{"identifier":3}"#).unwrap();
    assert_eq!(control.id, fast.id);
}

#[derive(Debug, PartialEq, Deserialize)]
struct ControlRequired {
    optional: Option<i64>,
    required: i64,
}

#[derive(Debug, PartialEq, SonicDeserialize)]
#[sonic(force_phf)]
struct FastRequired {
    optional: Option<i64>,
    required: i64,
}

#[test]
fn missing_option_and_required_field_match_serde() {
    let control: ControlRequired = sonic_rs::from_str(r#"{"required":9}"#).unwrap();
    let fast: FastRequired = sonic_rs::from_str(r#"{"required":9}"#).unwrap();
    assert_eq!(control.optional, fast.optional);
    assert_eq!(control.required, fast.required);

    let control = sonic_rs::from_str::<ControlRequired>("{}")
        .unwrap_err()
        .to_string();
    let fast = sonic_rs::from_str::<FastRequired>("{}")
        .unwrap_err()
        .to_string();
    assert!(control.contains("missing field"), "{control}");
    assert!(fast.contains("missing field"), "{fast}");
}

#[test]
fn all_sonic_entrypoints_keep_their_existing_interface() {
    let json = br#"{"user_id":7,"f31":31}"#;
    let from_slice: FastWide = sonic_rs::from_slice(json).unwrap();
    let from_reader: FastWide = sonic_rs::from_reader(&json[..]).unwrap();
    let value: sonic_rs::Value = sonic_rs::from_slice(json).unwrap();
    let from_value: FastWide = sonic_rs::from_value(&value).unwrap();
    assert_eq!(from_slice, from_reader);
    assert_eq!(from_slice, from_value);

    let escaped: FastWide = sonic_rs::from_str(r#"{"\u0075id":11}"#).unwrap();
    assert_eq!(escaped.user_id, 11);

    let unchecked: FastWide = unsafe { sonic_rs::from_slice_unchecked(json) }.unwrap();
    assert_eq!(unchecked, from_slice);

    let mut stream =
        sonic_rs::Deserializer::from_json(r#"{"user_id":1} {"uid":2}"#).into_stream::<FastWide>();
    assert_eq!(stream.next().unwrap().unwrap().user_id, 1);
    assert_eq!(stream.next().unwrap().unwrap().user_id, 2);
}

#[test]
fn sequence_representation_matches_serde() {
    let control: ControlRequired = sonic_rs::from_str("[null,9]").unwrap();
    let fast: FastRequired = sonic_rs::from_str("[null,9]").unwrap();
    assert_eq!(control.optional, fast.optional);
    assert_eq!(control.required, fast.required);
}

#[derive(Debug, Default, PartialEq, SonicDeserialize)]
#[serde(default)]
#[sonic(force_phf)]
struct FastNumericFields {
    #[serde(alias = "alias")]
    first: i64,
    second: i64,
}

#[test]
fn numeric_field_indexes_include_alias_entries() {
    let input = vec![(2_u64, 20_i64)].into_iter();
    let fast = FastNumericFields::deserialize(serde::de::value::MapDeserializer::<
        _,
        serde::de::value::Error,
    >::new(input))
    .unwrap();

    assert_eq!(
        fast,
        FastNumericFields {
            first: 0,
            second: 20
        }
    );
}

#[derive(Debug, Deserialize)]
struct ControlCustomRequired {
    #[serde(deserialize_with = "number_or_string")]
    value: i64,
}

#[derive(Debug, SonicDeserialize)]
#[sonic(force_phf)]
struct FastCustomRequired {
    #[serde(deserialize_with = "number_or_string")]
    value: i64,
}

#[test]
fn required_custom_decoder_is_not_called_for_a_missing_field() {
    let control = sonic_rs::from_str::<ControlCustomRequired>("{}")
        .unwrap_err()
        .to_string();
    let fast = sonic_rs::from_str::<FastCustomRequired>("{}")
        .unwrap_err()
        .to_string();
    assert!(control.contains("missing field"), "{control}");
    assert!(fast.contains("missing field"), "{fast}");

    let control: ControlCustomRequired = sonic_rs::from_str(r#"{"value":"5"}"#).unwrap();
    let fast: FastCustomRequired = sonic_rs::from_str(r#"{"value":"5"}"#).unwrap();
    assert_eq!(control.value, fast.value);
}

fn measure<T>(input: &str, iterations: usize) -> f64
where
    T: serde::de::DeserializeOwned,
{
    let start = Instant::now();
    for _ in 0..iterations {
        let value: T = sonic_rs::from_str(black_box(input)).unwrap();
        black_box(value);
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}

fn measure_serde_json<T>(input: &str, iterations: usize) -> f64
where
    T: serde::de::DeserializeOwned,
{
    let start = Instant::now();
    for _ in 0..iterations {
        let value: T = serde_json::from_str(black_box(input)).unwrap();
        black_box(value);
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}

/// Local, dependency-free benchmark. Run in release mode with one pinned CPU:
///
/// `taskset -c 2 cargo test --release --features derive --test sonic_deserialize benchmark_wide --
/// --ignored --nocapture`
#[test]
#[ignore]
fn benchmark_wide_struct_field_dispatch() {
    let input = wide_payload();
    let iterations = std::env::var("SONIC_DERIVE_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000usize);

    let control: ControlWide = sonic_rs::from_str(&input).unwrap();
    let fast: FastWide = sonic_rs::from_str(&input).unwrap();
    assert_same(&control, &fast);

    for sample in 0..6 {
        let (sonic_baseline, sonic_optimized, serde_json_baseline, serde_json_optimized) =
            if sample % 2 == 0 {
                (
                    measure::<ControlWide>(&input, iterations),
                    measure::<FastWide>(&input, iterations),
                    measure_serde_json::<ControlWide>(&input, iterations),
                    measure_serde_json::<FastWide>(&input, iterations),
                )
            } else {
                let serde_json_optimized = measure_serde_json::<FastWide>(&input, iterations);
                let serde_json_baseline = measure_serde_json::<ControlWide>(&input, iterations);
                let sonic_optimized = measure::<FastWide>(&input, iterations);
                let sonic_baseline = measure::<ControlWide>(&input, iterations);
                (
                    sonic_baseline,
                    sonic_optimized,
                    serde_json_baseline,
                    serde_json_optimized,
                )
            };
        println!(
            "sample={sample} sonic_baseline_ns_op={sonic_baseline:.2} \
             sonic_optimized_ns_op={sonic_optimized:.2} \
             serde_json_baseline_ns_op={serde_json_baseline:.2} \
             serde_json_optimized_ns_op={serde_json_optimized:.2}"
        );
    }
}
