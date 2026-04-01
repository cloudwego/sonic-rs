use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sonic_number::{parse_number, simd_str2int, swar_str2int, ParserNumber};

// ---- Test data ----

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
    "1234567890123456",
];

const INTEGERS: &[&str] = &[
    "0",
    "1",
    "42",
    "123",
    "9999",
    "12345",
    "123456",
    "1234567",
    "12345678",            // 8 digits — SWAR boundary
    "123456789",
    "1234567890",
    "12345678901234",      // 14 digits
    "1234567890123456",    // 16 digits — two SWAR batches
    "12345678901234567",   // 17 digits
    "1234567890123456789", // 19 digits — u64 range
];

fn int_label(s: &str) -> String {
    format!("{}d_{}", s.len(), s)
}

// ============================================================
// Group 1: Raw str2int primitives — length KNOWN
// ============================================================

fn bench_str2int_known(c: &mut Criterion) {
    let mut group = c.benchmark_group("str2int_known_len");

    for num_str in NUMBERS {
        let digits = num_str.len();
        let expected = num_str.parse::<u64>().unwrap();

        let mut padded = num_str.as_bytes().to_vec();
        padded.resize(padded.len().max(16), b' ');

        group.bench_with_input(BenchmarkId::new("simd", digits), &padded, |b, p| {
            b.iter(|| {
                let v = unsafe { simd_str2int(black_box(p), digits.min(16)) };
                debug_assert_eq!(v.0, expected);
                v
            })
        });

        let bytes = num_str.as_bytes();
        group.bench_with_input(BenchmarkId::new("swar", digits), &bytes, |b, s| {
            b.iter(|| {
                let v = unsafe { swar_str2int(black_box(*s), digits) };
                debug_assert_eq!(v.0, expected);
                v
            })
        });
    }

    group.finish();
}

// ============================================================
// Group 2: Raw str2int primitives — length UNKNOWN
//   Pass max need, let the function discover digit boundary.
// ============================================================

fn bench_str2int_unknown(c: &mut Criterion) {
    let mut group = c.benchmark_group("str2int_unknown_len");

    for num_str in NUMBERS {
        let digits = num_str.len();
        let expected = num_str.parse::<u64>().unwrap();

        // Pad to 20 bytes with spaces (non-digit terminator)
        let mut padded = num_str.as_bytes().to_vec();
        padded.resize(20, b' ');

        group.bench_with_input(BenchmarkId::new("simd", digits), &padded, |b, p| {
            b.iter(|| {
                let v = unsafe { simd_str2int(black_box(p), 16) };
                debug_assert_eq!(v.0, expected);
                v
            })
        });

        group.bench_with_input(BenchmarkId::new("swar", digits), &padded, |b, p| {
            b.iter(|| {
                let v = unsafe { swar_str2int(black_box(p), 19) };
                debug_assert_eq!(v.0, expected);
                v
            })
        });
    }

    group.finish();
}

// ============================================================
// Group 3: Full parse_number per integer length
// ============================================================

fn bench_parse_number(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_number");

    let inputs: Vec<(&str, Vec<u8>)> = INTEGERS
        .iter()
        .map(|s| {
            let mut data = s.as_bytes().to_vec();
            data.push(b' ');
            (*s, data)
        })
        .collect();

    for (label, data) in &inputs {
        let id = int_label(label);

        group.bench_with_input(BenchmarkId::new("int", &id), data, |b, data| {
            b.iter(|| {
                let mut index = 0;
                parse_number(black_box(data), &mut index, false)
            })
        });
    }

    group.finish();
}

// ============================================================
// Group 4: Full parse_number batch — aggregate throughput
// ============================================================

fn bench_parse_number_batch(c: &mut Criterion) {
    let inputs: Vec<Vec<u8>> = INTEGERS
        .iter()
        .map(|s| {
            let mut data = s.as_bytes().to_vec();
            data.push(b' ');
            data
        })
        .collect();

    c.bench_function("parse_number_batch", |b| {
        b.iter(|| {
            for data in &inputs {
                let mut index = 0;
                let _ = parse_number(black_box(data), &mut index, false);
            }
        })
    });
}

criterion_group!(
    benches,
    bench_str2int_known,
    bench_str2int_unknown,
    bench_parse_number,
    bench_parse_number_batch,
);
criterion_main!(benches);
