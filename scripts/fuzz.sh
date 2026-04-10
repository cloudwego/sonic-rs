#!/bin/bash

set -ex

cargo install cargo-fuzz

FUZZ_TIME="${FUZZ_TIME:-5m}"

TARGETS=(
    fuzz_value
    fuzz_number
    fuzz_string
    fuzz_get_path
    fuzz_deep_nesting
    fuzz_serde_roundtrip
)

# Run each target sequentially. Override FUZZ_TARGETS to select specific ones.
if [ -n "$FUZZ_TARGETS" ]; then
    IFS=',' read -ra TARGETS <<< "$FUZZ_TARGETS"
fi

for target in "${TARGETS[@]}"; do
    echo "=== Fuzzing $target for $FUZZ_TIME ==="
    RUST_BACKTRACE=full cargo +nightly fuzz run "$target" -- -max_total_time="$FUZZ_TIME"
done

echo "=== All fuzz targets completed ==="
