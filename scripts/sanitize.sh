#!/bin/bash

set -ex

export ASAN_OPTIONS="disable_coredump=0:unmap_shadow_on_exit=1:abort_on_error=1"

run_tests() {
    local san="$1"
    local features="$2"
    cargo +nightly test --target x86_64-unknown-linux-gnu --features "$features"  -- --test-threads=1 --nocapture
    cargo +nightly test --doc --package sonic-rs --target x86_64-unknown-linux-gnu --features "$features" -- --show-output --test-threads=1
}

for san in address leak; do
    for feature in "" "arbitrary_precision" "sort_keys" "use_raw" "utf8_lossy"; do
        echo "Running tests with $san and $feature"
        RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" run_tests $san $feature
    done
done
