#[macro_use]
extern crate criterion;

use criterion::SamplingMode;
use criterion::{criterion_group, BatchSize, Criterion, Throughput};
use std::fs::File;
use std::io::Read;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

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
    (json: $name:ident, structure: $structure:ty) => {
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
                let serde_val: $structure = serde_json::from_slice(&data).unwrap();
                let serde_out = serde_json::to_string_pretty(&serde_val).unwrap();

                let value: $structure = sonic_rs::from_slice(&data).unwrap();
                let out = sonic_rs::to_string_pretty(&value).unwrap();
                assert!(
                    diff_json(&out, &serde_out),
                    "sonic_rs failed in {}",
                    stringify!($name)
                );

                let mut data = data.clone();
                let value: $structure = simd_json::from_slice(&mut data).unwrap();
                let _out = simd_json::to_string_pretty(&value).unwrap();
                // assert!(
                //     diff_json(&out, &serde_out),
                //     "simdjson failed in {}",
                //     stringify!($name)
                // );
            }

            let mut group = c.benchmark_group(stringify!($name));
            group.sampling_mode(SamplingMode::Flat);

            let val: $structure = sonic_rs::from_slice(&data).unwrap();
            group.bench_with_input("sonic_rs::to_string", &val, |b, data| {
                b.iter_batched(
                    || data,
                    |val| sonic_rs::to_string(&val).unwrap(),
                    BatchSize::SmallInput,
                )
            });

            let mut data2 = data.clone();
            let val: $structure = simd_json::from_slice(&mut data2).unwrap();
            group.bench_with_input("simd_json::to_string", &val, |b, data| {
                b.iter_batched(
                    || data.clone(),
                    |val| simd_json::to_string(&val).unwrap(),
                    BatchSize::SmallInput,
                )
            });

            let val: $structure = serde_json::from_slice(&data).unwrap();
            group.bench_with_input("serde_json::to_string", &val, |b, data| {
                b.iter_batched(
                    || data,
                    |val| serde_json::to_string(&val).unwrap(),
                    BatchSize::SmallInput,
                )
            });

            group.throughput(Throughput::Bytes(data.len() as u64));
        }
    };
}

use json_benchmark::canada::Canada;
use json_benchmark::copy::{citm_catalog::CitmCatalog, twitter::Twitter};

bench_file!(
    json: twitter,
    structure: Twitter
);
bench_file!(
    json: canada,
    structure: Canada
);
bench_file!(
    json: citm_catalog,
    structure: CitmCatalog
);

criterion_group!(benches, twitter, canada, citm_catalog,);
criterion_main!(benches);
