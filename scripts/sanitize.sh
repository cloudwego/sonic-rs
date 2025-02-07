#!/bin/bash

set -ex

export ASAN_OPTIONS="disable_coredump=0:unmap_shadow_on_exit=1:abort_on_error=1"

run_tests() {
    cargo +nightly test --release --target x86_64-unknown-linux-gnu --features sanitize,"$1"
    cargo +nightly test --doc --package sonic-rs --target x86_64-unknown-linux-gnu --features sanitize,"$1"
}

echo "Running tests with $1 and $2"
RUSTFLAGS="-Zsanitizer=$1" RUSTDOCFLAGS="-Zsanitizer=$1" run_tests "$2"
