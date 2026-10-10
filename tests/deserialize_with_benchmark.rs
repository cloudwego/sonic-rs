#![cfg(feature = "derive")]

use std::{fmt::Write, hint::black_box, time::Instant};

use serde::{Deserialize, Deserializer, Serialize};
use sonic_rs::SonicDeserialize;

fn identity_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    i64::deserialize(deserializer)
}

macro_rules! define_plain {
    ($name:ident, $deserialize:path) => {
        #[derive(Debug, PartialEq, Serialize, $deserialize)]
        struct $name {
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
        }
    };
}

macro_rules! define_with {
    ($name:ident, $deserialize:path) => {
        #[derive(Debug, PartialEq, Serialize, $deserialize)]
        struct $name {
            #[serde(deserialize_with = "identity_i64")]
            f01: i64,
            #[serde(deserialize_with = "identity_i64")]
            f02: i64,
            #[serde(deserialize_with = "identity_i64")]
            f03: i64,
            #[serde(deserialize_with = "identity_i64")]
            f04: i64,
            #[serde(deserialize_with = "identity_i64")]
            f05: i64,
            #[serde(deserialize_with = "identity_i64")]
            f06: i64,
            #[serde(deserialize_with = "identity_i64")]
            f07: i64,
            #[serde(deserialize_with = "identity_i64")]
            f08: i64,
            #[serde(deserialize_with = "identity_i64")]
            f09: i64,
            #[serde(deserialize_with = "identity_i64")]
            f10: i64,
            #[serde(deserialize_with = "identity_i64")]
            f11: i64,
            #[serde(deserialize_with = "identity_i64")]
            f12: i64,
            #[serde(deserialize_with = "identity_i64")]
            f13: i64,
            #[serde(deserialize_with = "identity_i64")]
            f14: i64,
            #[serde(deserialize_with = "identity_i64")]
            f15: i64,
            #[serde(deserialize_with = "identity_i64")]
            f16: i64,
            #[serde(deserialize_with = "identity_i64")]
            f17: i64,
            #[serde(deserialize_with = "identity_i64")]
            f18: i64,
            #[serde(deserialize_with = "identity_i64")]
            f19: i64,
            #[serde(deserialize_with = "identity_i64")]
            f20: i64,
            #[serde(deserialize_with = "identity_i64")]
            f21: i64,
            #[serde(deserialize_with = "identity_i64")]
            f22: i64,
            #[serde(deserialize_with = "identity_i64")]
            f23: i64,
            #[serde(deserialize_with = "identity_i64")]
            f24: i64,
            #[serde(deserialize_with = "identity_i64")]
            f25: i64,
            #[serde(deserialize_with = "identity_i64")]
            f26: i64,
            #[serde(deserialize_with = "identity_i64")]
            f27: i64,
            #[serde(deserialize_with = "identity_i64")]
            f28: i64,
            #[serde(deserialize_with = "identity_i64")]
            f29: i64,
            #[serde(deserialize_with = "identity_i64")]
            f30: i64,
            #[serde(deserialize_with = "identity_i64")]
            f31: i64,
            #[serde(deserialize_with = "identity_i64")]
            f32: i64,
            #[serde(deserialize_with = "identity_i64")]
            f33: i64,
            #[serde(deserialize_with = "identity_i64")]
            f34: i64,
            #[serde(deserialize_with = "identity_i64")]
            f35: i64,
            #[serde(deserialize_with = "identity_i64")]
            f36: i64,
            #[serde(deserialize_with = "identity_i64")]
            f37: i64,
            #[serde(deserialize_with = "identity_i64")]
            f38: i64,
            #[serde(deserialize_with = "identity_i64")]
            f39: i64,
            #[serde(deserialize_with = "identity_i64")]
            f40: i64,
            #[serde(deserialize_with = "identity_i64")]
            f41: i64,
            #[serde(deserialize_with = "identity_i64")]
            f42: i64,
            #[serde(deserialize_with = "identity_i64")]
            f43: i64,
            #[serde(deserialize_with = "identity_i64")]
            f44: i64,
            #[serde(deserialize_with = "identity_i64")]
            f45: i64,
            #[serde(deserialize_with = "identity_i64")]
            f46: i64,
            #[serde(deserialize_with = "identity_i64")]
            f47: i64,
            #[serde(deserialize_with = "identity_i64")]
            f48: i64,
            #[serde(deserialize_with = "identity_i64")]
            f49: i64,
            #[serde(deserialize_with = "identity_i64")]
            f50: i64,
            #[serde(deserialize_with = "identity_i64")]
            f51: i64,
            #[serde(deserialize_with = "identity_i64")]
            f52: i64,
            #[serde(deserialize_with = "identity_i64")]
            f53: i64,
            #[serde(deserialize_with = "identity_i64")]
            f54: i64,
            #[serde(deserialize_with = "identity_i64")]
            f55: i64,
            #[serde(deserialize_with = "identity_i64")]
            f56: i64,
            #[serde(deserialize_with = "identity_i64")]
            f57: i64,
            #[serde(deserialize_with = "identity_i64")]
            f58: i64,
            #[serde(deserialize_with = "identity_i64")]
            f59: i64,
            #[serde(deserialize_with = "identity_i64")]
            f60: i64,
            #[serde(deserialize_with = "identity_i64")]
            f61: i64,
            #[serde(deserialize_with = "identity_i64")]
            f62: i64,
            #[serde(deserialize_with = "identity_i64")]
            f63: i64,
            #[serde(deserialize_with = "identity_i64")]
            f64: i64,
        }
    };
}

define_plain!(PlainSerde, Deserialize);
define_plain!(PlainSonic, SonicDeserialize);
define_with!(WithSerde, Deserialize);
define_with!(WithSonic, SonicDeserialize);

fn payload() -> String {
    let mut json = String::from("{");
    for index in 1..=64 {
        if index > 1 {
            json.push(',');
        }
        write!(&mut json, "\"f{index:02}\":{index}").unwrap();
    }
    json.push('}');
    json
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

#[test]
#[ignore]
fn benchmark_deserialize_with_overhead() {
    let input = payload();
    let iterations = std::env::var("SONIC_DERIVE_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000usize);

    let plain_serde: PlainSerde = sonic_rs::from_str(&input).unwrap();
    let plain_sonic: PlainSonic = sonic_rs::from_str(&input).unwrap();
    let with_serde: WithSerde = sonic_rs::from_str(&input).unwrap();
    let with_sonic: WithSonic = sonic_rs::from_str(&input).unwrap();
    assert_eq!(
        serde_json::to_value(&plain_serde).unwrap(),
        serde_json::to_value(&plain_sonic).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&with_serde).unwrap(),
        serde_json::to_value(&with_sonic).unwrap()
    );

    for sample in 0..6 {
        let plain_serde = measure::<PlainSerde>(&input, iterations);
        let plain_sonic = measure::<PlainSonic>(&input, iterations);
        let with_serde = measure::<WithSerde>(&input, iterations);
        let with_sonic = measure::<WithSonic>(&input, iterations);

        println!(
            "sample={sample} plain_serde_ns_op={plain_serde:.2} \
             plain_sonic_ns_op={plain_sonic:.2} with_serde_ns_op={with_serde:.2} \
             with_sonic_ns_op={with_sonic:.2}"
        );
    }
}
