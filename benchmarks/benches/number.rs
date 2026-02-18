// Benchmark comparison between atoi_simd and sonic-number
use std::hint::black_box;

use atoi_simd::parse as atoi_simd_parse;
use criterion::{Criterion, criterion_group, criterion_main};
use sonic_number::simd_str2int;

const NUMBERS: &[&str] = &[
    "1",
    "12",
    "123",
    "1234",
    "12345",
    "123456",
    "1234567",
    "12345678",
    "123456789",
    "1234567890",
    "12345678901",
    "123456789012",
    "1234567890123",
    "12345678901234",
    "123456789012345",
    "1234567890123456", // max 16 digits for `simd_str2int`
];

fn bench_i64(c: &mut Criterion) {
    let expected = NUMBERS
        .iter()
        .map(|s| s.parse::<u64>().unwrap())
        .collect::<Vec<_>>();

    c.bench_function("sonic-number parse", |b| {
        b.iter(|| {
            for (s, expected) in NUMBERS.iter().zip(expected.iter()) {
                let val = unsafe { simd_str2int(black_box(s.as_bytes()), s.len()) };
                assert_eq!(val.0, *expected);
            }
        })
    });

    c.bench_function("atoi_simd parse", |b| {
        b.iter(|| {
            for (s, expected) in NUMBERS.iter().zip(expected.iter()) {
                let val = atoi_simd_parse::<u64, false, false>(black_box(s.as_bytes())).unwrap();
                assert_eq!(val, *expected);
            }
        })
    });
}

criterion_group!(benches, bench_i64);
criterion_main!(benches);
