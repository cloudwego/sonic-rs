#[macro_use]
extern crate criterion;

use criterion::SamplingMode;
use criterion::{criterion_group, BatchSize, Criterion, Throughput};
use std::fs::File;
use std::io::Read;
use std::str::from_utf8_unchecked;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn simdjson_to_borrowed_value(data: &mut [u8]) {
    let _ = simd_json::to_borrowed_value(data).unwrap();
}

fn simdjson_to_owned_value(data: &mut [u8]) {
    let _ = simd_json::to_owned_value(data).unwrap();
}

fn serde_from_slice(data: &[u8]) {
    let _: serde_json::Value = serde_json::from_slice(data).unwrap();
}

fn serde_from_str(data: &[u8]) {
    let data = unsafe { from_utf8_unchecked(data) };
    let _: serde_json::Value = serde_json::from_str(data).unwrap();
}

fn sonic_rs_from_slice(data: &[u8]) {
    let _: sonic_rs::Value = sonic_rs::from_slice(data).unwrap();
}

fn sonic_rs_from_slice_unchecked(data: &[u8]) {
    let _: sonic_rs::Value = unsafe { sonic_rs::from_slice_unchecked(data).unwrap() };
}

// fn sonic_rs_skip_one(data: &[u8]) {
//     unsafe {
//         let data = from_utf8_unchecked(data);
//         let empty: &[&str] = &[];
//         let _ = sonic_rs::get_unchecked(data, empty).unwrap();
//     }
// }

// fn sonic_rs_to_serdejson_value(data: &[u8]) {
//     let _: serde_json::Value = sonic_rs::from_slice(data).unwrap();
// }

macro_rules! bench_file {
    ($name:ident) => {
        #[allow(unused)]
        fn $name(c: &mut Criterion) {
            let core_ids = core_affinity::get_core_ids().unwrap();
            core_affinity::set_for_current(core_ids[0]);

            let mut vec = Vec::new();
            let root = env!("CARGO_MANIFEST_DIR").to_owned();
            File::open(root + concat!("/benches/testdata/", stringify!($name), ".json"))
                .unwrap()
                .read_to_end(&mut vec)
                .unwrap();

            // verify sonic-rs parse
            let serde_out: serde_json::Value = serde_json::from_slice(&vec).unwrap();

            let value: sonic_rs::Value = sonic_rs::from_slice(&vec).unwrap();
            let out = sonic_rs::to_string(&value).unwrap();
            let rs_out1: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(rs_out1, serde_out);

            let mut group = c.benchmark_group(stringify!($name));
            group.sampling_mode(SamplingMode::Flat);

            group.bench_with_input("sonic_rs_dom::from_slice", &vec, |b, data| {
                b.iter_batched(
                    || data,
                    |bytes| sonic_rs_from_slice(&bytes),
                    BatchSize::SmallInput,
                )
            });

            group.bench_with_input("sonic_rs_dom::from_slice_unchecked", &vec, |b, data| {
                b.iter_batched(
                    || data,
                    |bytes| sonic_rs_from_slice_unchecked(&bytes),
                    BatchSize::SmallInput,
                )
            });

            // group.bench_with_input("sonic_rs::skip_one", &vec, |b, data| {
            //     b.iter_batched(
            //         || data,
            //         |bytes| sonic_rs_skip_one(&bytes),
            //         BatchSize::SmallInput,
            //     )
            // });

            // group.bench_with_input("sonic_rs::to_serdejson_value", &vec, |b, data| {
            //     b.iter_batched(
            //         || data,
            //         |bytes| sonic_rs_to_serdejson_value(&bytes),
            //         BatchSize::SmallInput,
            //     )
            // });

            group.bench_with_input("serde_json::from_slice", &vec, |b, data| {
                b.iter_batched(
                    || data,
                    |bytes| serde_from_slice(&bytes),
                    BatchSize::SmallInput,
                )
            });

            group.bench_with_input("serde_json::from_str", &vec, |b, data| {
                b.iter_batched(
                    || data,
                    |bytes| serde_from_str(&bytes),
                    BatchSize::SmallInput,
                )
            });

            group.bench_with_input("simd_json::slice_to_owned_value", &vec, |b, data| {
                b.iter_batched(
                    || data.clone(),
                    |mut bytes| simdjson_to_owned_value(&mut bytes),
                    BatchSize::SmallInput,
                )
            });

            group.bench_with_input("simd_json::slice_to_borrowed_value", &vec, |b, data| {
                b.iter_batched(
                    || data.clone(),
                    |mut bytes| simdjson_to_borrowed_value(&mut bytes),
                    BatchSize::SmallInput,
                )
            });
            group.throughput(Throughput::Bytes(vec.len() as u64));
        }
    };
}

bench_file!(book);
bench_file!(canada);
bench_file!(citm_catalog);
bench_file!(twitter);
bench_file!(github_events);

// criterion_group!(benches, canada, otfcc, citm_catalog, twitter, lottie, github_events, twitterescaped, book, poet, fgo);
criterion_group!(benches, twitter, canada, citm_catalog);
criterion_main!(benches);
