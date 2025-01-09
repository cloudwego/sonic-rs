#[macro_use]
extern crate criterion;
use std::io::Read;

use criterion::{criterion_group, BatchSize, Criterion};
use sonic_rs::{JsonValueMutTrait, JsonValueTrait};

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

fn bench_value_clone(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("value");
    group.bench_with_input("sonic-rs::value_clone", &sonic_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                let _ = value.clone();
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("serde_json::value_clone", &serde_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                let _ = value.clone();
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_convert_value(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("value");
    group.bench_with_input("sonic-rs::value_convert", &sonic_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                let _: serde_json::Value = value.clone().into();
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("serde_json::value_convert", &serde_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                let _: sonic_rs::Value = value.clone().into();
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_modify_and_clone(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("value");
    group.bench_with_input(
        "sonic-rs::value_modify_and_clone",
        &sonic_value,
        |b, data| {
            b.iter_batched(
                || data.clone(),
                |mut value| {
                    for i in 0..10 {
                        value.as_object_mut().unwrap().insert("inserted", i);
                        let _ = value.clone();
                    }
                },
                BatchSize::SmallInput,
            )
        },
    );

    group.bench_with_input(
        "serde_json::value_modify_and_clone",
        &serde_value,
        |b, data| {
            b.iter_batched(
                || data.clone(),
                |mut value| {
                    for i in 0..10 {
                        value
                            .as_object_mut()
                            .unwrap()
                            .insert("inserted".to_string(), serde_json::Value::from(i));
                        let _ = value.clone();
                    }
                },
                BatchSize::SmallInput,
            )
        },
    );
}

fn bench_object_insert(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("value");
    group.bench_function("sonic-rs::object_insert", |b| {
        b.iter_batched(
            || (),
            |_| {
                let mut val = sonic_rs::Object::new();
                for i in 0..100 {
                    let mut obj = sonic_rs::json!({"a":{"b":{"c":{"d":{}}}}});
                    for j in 0..100 {
                        obj["a"]["b"]["c"]["d"]
                            .as_object_mut()
                            .unwrap()
                            .insert(&j.to_string(), true);
                    }
                    val.insert(&i.to_string(), obj);
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("serde_json::object_insert", |b| {
        b.iter_batched(
            || (),
            |_| {
                let mut val = serde_json::Map::new();
                for i in 0..100 {
                    let mut obj = serde_json::json!({"a":{"b":{"c":{"d":{}}}}});
                    for j in 0..100 {
                        obj["a"]["b"]["c"]["d"]
                            .as_object_mut()
                            .unwrap()
                            .insert(j.to_string(), serde_json::Value::Bool(true));
                    }
                    val.insert(i.to_string(), obj);
                }
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_object_get(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut sonic_value = sonic_rs::Object::new();
    for i in 0..100 {
        let mut obj = sonic_rs::json!({"a":{"b":{"c":{"d":{}}}}});
        for j in 0..100 {
            obj["a"]["b"]["c"]["d"]
                .as_object_mut()
                .unwrap()
                .insert(&j.to_string(), true);
        }
        sonic_value.insert(&i.to_string(), obj);
    }

    let mut serde_value = serde_json::Map::new();
    for i in 0..100 {
        let mut obj = serde_json::json!({"a":{"b":{"c":{"d":{}}}}});
        for j in 0..100 {
            obj["a"]["b"]["c"]["d"]
                .as_object_mut()
                .unwrap()
                .insert(j.to_string(), serde_json::Value::Bool(true));
        }
        serde_value.insert(i.to_string(), obj);
    }

    let mut group = c.benchmark_group("value");
    group.bench_with_input("sonic-rs::object_get", &sonic_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                for i in 0..100 {
                    for j in 0..100 {
                        let _ = value[&i.to_string()]["a"]["b"]["c"]["d"][&j.to_string()].as_bool();
                    }
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("serde_json::object_get", &serde_value, |b, data| {
        b.iter_batched(
            || data,
            |value| {
                for i in 0..100 {
                    for j in 0..100 {
                        let _ = value[&i.to_string()]["a"]["b"]["c"]["d"][&j.to_string()].as_bool();
                    }
                }
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_get,
    bench_value_clone,
    bench_convert_value,
    bench_modify_and_clone,
    bench_object_insert,
    bench_object_get
);
criterion_main!(benches);
