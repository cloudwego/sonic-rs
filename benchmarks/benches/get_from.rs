#[macro_use]
extern crate criterion;
use std::io::Read;

use criterion::{criterion_group, BatchSize, Criterion};

fn bench_get(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut data = Vec::new();
    let root = env!("CARGO_MANIFEST_DIR").to_owned();
    std::fs::File::open(root + concat!("/benches/testdata/twitter.json"))
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    let data = unsafe { std::str::from_utf8_unchecked(&data) };

    // verify sonic-rs parse
    let rpath = ["search_metadata", "count"];
    let gpath = "search_metadata.count";
    let gout = gjson::get(data, gpath);
    let rout = unsafe { sonic_rs::get_unchecked(data, &rpath) };
    assert_eq!(rout.unwrap().as_raw_str(), gout.str());

    let mut group = c.benchmark_group("twitter");

    group.bench_with_input("sonic-rs::get_unchecked_from_str", data, |b, data| {
        b.iter_batched(
            || data,
            |json| unsafe { sonic_rs::get_unchecked(json, &rpath) },
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("sonic-rs::get_from_str", data, |b, data| {
        b.iter_batched(
            || data,
            |json| sonic_rs::get(json, &rpath),
            BatchSize::SmallInput,
        )
    });

    group.bench_with_input("gjson::get_from_str", data, |b, data| {
        b.iter_batched(
            || data,
            |json| gjson::get(json, gpath),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_get);
criterion_main!(benches);
