#[macro_use]
extern crate criterion;

use std::{fs::File, io::Read};

use criterion::{criterion_group, BatchSize, Criterion, SamplingMode, Throughput};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn simdjson_to_string(val: &simd_json::value::borrowed::Value) {
    let _ = simd_json::to_string(val).unwrap();
}

fn serde_to_string(val: &serde_json::Value) {
    let _ = serde_json::to_string(val).unwrap();
}

fn sonic_rs_to_string(val: &sonic_rs::Value) {
    let _ = sonic_rs::to_string(val).unwrap();
}

fn diff_json(got: &str, expect: &str) -> bool {
    let value1: serde_json::Value = serde_json::from_str(got).unwrap();
    let value2: serde_json::Value = serde_json::from_str(expect).unwrap();

    fn write_to(file: &str, data: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(file)?;
        file.write_all(data.as_bytes())?;
        Ok(())
    }

    if value1 != value2 {
        write_to("got.json", got).unwrap();
        write_to("expect.json", expect).unwrap();
        false
    } else {
        true
    }
}

macro_rules! bench_file {
    ($name:ident) => {
        #[allow(unused)]
        fn $name(c: &mut Criterion) {
            let core_ids = core_affinity::get_core_ids().unwrap();
            core_affinity::set_for_current(core_ids[0]);

            let mut data = Vec::new();
            let root = env!("CARGO_MANIFEST_DIR").to_owned();
            File::open(root + concat!("/benches/testdata/", stringify!($name), ".json"))
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();

            // verify sonic-rs parse
            if stringify!($name) != "canada" {
                let serde_out: serde_json::Value = serde_json::from_slice(&data).unwrap();
                let expect = serde_json::to_string(&serde_out).unwrap();

                let value: sonic_rs::Value = sonic_rs::from_slice(&data).unwrap();
                let got = sonic_rs::to_string(&value).unwrap();
                assert!(
                    diff_json(&got, &expect),
                    concat!("/benches/testdata/", stringify!($name))
                );
            }

            let mut group = c.benchmark_group(stringify!($name));
            group.sampling_mode(SamplingMode::Flat);

            let value: sonic_rs::Value = sonic_rs::from_slice(&data).unwrap();
            group.bench_with_input("sonic_rs::to_string", &value, |b, data| {
                b.iter_batched(
                    || data,
                    |val| sonic_rs_to_string(&val),
                    BatchSize::SmallInput,
                )
            });

            let value: serde_json::Value = serde_json::from_slice(&data).unwrap();
            group.bench_with_input("serde_json::to_string", &value, |b, data| {
                b.iter_batched(|| data, |val| serde_to_string(&val), BatchSize::SmallInput)
            });

            let mut copy = data.clone();
            let value = simd_json::to_borrowed_value(&mut copy).unwrap();
            group.bench_with_input("simd_json::to_string", &value, |b, data| {
                b.iter_batched(
                    || data.clone(),
                    |val| simdjson_to_string(&val),
                    BatchSize::SmallInput,
                )
            });

            group.throughput(Throughput::Bytes(data.len() as u64));
        }
    };
}

bench_file!(book);
bench_file!(canada);
bench_file!(citm_catalog);
bench_file!(twitter);
bench_file!(github_events);

// criterion_group!(benches, canada, otfcc, citm_catalog, twitter, lottie, github_events,
// twitterescaped, book, poet, fgo);
criterion_group!(benches, twitter, citm_catalog, canada);
criterion_main!(benches);
