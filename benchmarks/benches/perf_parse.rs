use std::{env, fs, hint::black_box};

fn main() {
    // Pin to core 0 for stable measurements
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let args: Vec<String> = env::args().collect();
    let file = args.get(1).expect("usage: perf_parse <json_file> [iters]");
    let iters: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);

    let data = fs::read(file).unwrap();
    eprintln!("file: {} ({} bytes), iters: {}", file, data.len(), iters);

    for _ in 0..iters {
        let v: sonic_rs::Value = sonic_rs::from_slice(black_box(&data)).unwrap();
        black_box(&v);
    }
}
