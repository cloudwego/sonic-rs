#[macro_use]
extern crate criterion;

use criterion::SamplingMode;
use criterion::{criterion_group, BatchSize, Criterion, Throughput};
use std::fs::File;
use std::io::Read;
use std::str::from_utf8_unchecked;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn serde_json_parse_struct<'de, T>(data: &'de [u8]) -> serde_json::Result<T>
where
    T: serde::Deserialize<'de>,
{
    serde_json::from_slice::<T>(data)
}

fn serde_json_parse_struct_from_str<'de, T>(data: &'de [u8]) -> serde_json::Result<T>
where
    T: serde::Deserialize<'de>,
{
    let data = unsafe { from_utf8_unchecked(data) };
    serde_json::from_str::<T>(data)
}

fn sonic_rs_parse_struct<'de, T>(data: &'de [u8]) -> sonic_rs::Result<T>
where
    T: serde::Deserialize<'de>,
{
    sonic_rs::from_slice::<T>(data)
}

fn simd_json_parse_struct<'de, T>(data: &'de mut [u8]) -> simd_json::Result<T>
where
    T: serde::Deserialize<'de>,
{
    simd_json::serde::from_slice::<T>(data)
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
    (json: $name:ident, structure: $structure:ty) => {
        paste::item! {
            #[allow(non_snake_case)]
            fn [< bench_ $name _ $structure >](c: &mut Criterion) {
                let core_ids = core_affinity::get_core_ids().unwrap();
                core_affinity::set_for_current(core_ids[0]);

                let mut vec = Vec::new();
                let root = env!("CARGO_MANIFEST_DIR").to_owned();
                File::open(root + concat!("/benches/testdata/", stringify!($name), ".json"))
                    .unwrap()
                    .read_to_end(&mut vec)
                    .unwrap();

                // verify sonic-rs parse
                let serde_val: $structure = serde_json::from_slice(&vec).unwrap();
                let serde_out = serde_json::to_string_pretty(&serde_val).unwrap();

                let value : $structure = sonic_rs::from_slice(&vec).unwrap();
                let out = sonic_rs::to_string_pretty(&value).unwrap();
                assert!(diff_json(&out, &serde_out));

                let mut group = c.benchmark_group(stringify!($name));
                group.sampling_mode(SamplingMode::Flat);

                group.bench_with_input("sonic_rs::from_slice", &vec, |b, data| {
                    b.iter_batched(
                        || data,
                        |bytes| sonic_rs_parse_struct::<$structure>(&bytes),
                        BatchSize::SmallInput,
                    )
                });

                group.bench_with_input("simd_json::from_slice", &vec, |b, data| {
                    b.iter_batched(
                        || data.clone(),
                        |mut bytes| simd_json_parse_struct::<$structure>(&mut bytes),
                        BatchSize::SmallInput,
                    )
                });

                group.bench_with_input("serde_json::from_slice", &vec, |b, data| {
                    b.iter_batched(
                        || data,
                        |bytes| serde_json_parse_struct::<$structure>(&bytes),
                        BatchSize::SmallInput,
                    )
                });

                group.bench_with_input("serde_json::from_str", &vec, |b, data| {
                    b.iter_batched(
                        || data,
                        |bytes| serde_json_parse_struct_from_str::<$structure>(&bytes),
                        BatchSize::SmallInput,
                    )
                });

                group.throughput(Throughput::Bytes(vec.len() as u64));
            }
        }
    };
}

use json_benchmark::{citm_catalog::CitmCatalog, twitter::Twitter};

use json_benchmark::canada::Canada;

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

criterion_group!(
    benches,
    bench_twitter_Twitter,
    bench_citm_catalog_CitmCatalog,
    bench_canada_Canada,
);
criterion_main!(benches);
