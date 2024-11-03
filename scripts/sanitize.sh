#!/bin/bash

set -ex

export ASAN_OPTIONS="disable_coredump=0:unmap_shadow_on_exit=1:abort_on_error=1"

for san in address leak; do
    echo "Running tests with $san"
    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo +nightly test --target x86_64-unknown-linux-gnu -- --test-threads=1
    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo +nightly test --doc --package sonic-rs --target x86_64-unknown-linux-gnu -- --show-output --test-threads=1

    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo +nightly test  --features arbitrary_precision --target x86_64-unknown-linux-gnu -- --test-threads=1
    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo +nightly test  --features arbitrary_precision --doc --package sonic-rs --target x86_64-unknown-linux-gnu -- --show-output --test-threads=1


    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo +nightly test --features use_raw --target x86_64-unknown-linux-gnu -- --test-threads=1
    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo +nightly test --features use_raw --doc --package sonic-rs --target x86_64-unknown-linux-gnu -- --show-output --test-threads=1
done


