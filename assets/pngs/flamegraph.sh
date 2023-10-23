
# the command to profiling sonic-rs benchmarks

CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --bench  deserialize_struct -- --bench citm_catalog/sonic  --profile-time 5

CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --bench  deserialize_struct -- --bench citm_catalog/simd_json  --profile-time 5