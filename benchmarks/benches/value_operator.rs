#[macro_use]
extern crate criterion;
use std::io::Read;

use criterion::{criterion_group, BatchSize, Criterion};
use sonic_rs::JsonValueTrait;

fn bench_get(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut data = Vec::new();
    let root = env!("CARGO_MANIFEST_DIR").to_owned();
    std::fs::File::open(root + concat!("/benches/testdata/twitter.json"))
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();

    let sonic_value: sonic_rs::Value = sonic_rs::from_slice(&data).unwrap();
    let serde_value: serde_json::Value = serde_json::from_slice(&data).unwrap();

    assert_eq!(
        sonic_value["statuses"][4]["entities"]["media"][0]["source_status_id_str"].as_str(),
        Some("439430848190742528")
    );
    assert_eq!(
        serde_value["statuses"][4]["entities"]["media"][0]["source_status_id_str"].as_str(),
        Some("439430848190742528")
    );

    let mut group = c.benchmark_group("value");
    group.bench_with_input("sonic-rs::value_get", &sonic_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                let _ =
                    value["statuses"][4]["entities"]["media"][0]["source_status_id_str"].as_str();
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("serde_json::value_get", &serde_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                let _ =
                    value["statuses"][4]["entities"]["media"][0]["source_status_id_str"].as_str();
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("sonic_rs::value_new", &sonic_value, |b, data| {
        b.iter_batched(
            || data,
            |_value| {
                let mut value = sonic_rs::Array::new();
                for i in 0..100 {
                    value.push(i);
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("serde_json::value_new", &serde_value, |b, data| {
        b.iter_batched(
            || data,
            |_value| {
                let mut value = serde_json::Value::Array(Vec::new());
                let array = &mut value.as_array_mut().unwrap();
                for i in 0..100 {
                    array.push(serde_json::Value::from(i as f64));
                }
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_get);
criterion_main!(benches);
