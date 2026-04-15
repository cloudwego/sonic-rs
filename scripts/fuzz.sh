#!/bin/bash

set -ex

cargo install cargo-fuzz

FUZZ_TIME="${FUZZ_TIME:-5m}"

normalize_duration_to_seconds() {
    case "$1" in
        *h) echo $(( ${1%h} * 3600 )) ;;
        *m) echo $(( ${1%m} * 60 )) ;;
        *s) echo $(( ${1%s} )) ;;
        *) echo "$1" ;;
    esac
}

FUZZ_TIME_SECONDS="$(normalize_duration_to_seconds "$FUZZ_TIME")"

TARGETS=(
    fuzz_value
    fuzz_number
    fuzz_f32_literal
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
    echo "=== Fuzzing $target for $FUZZ_TIME (${FUZZ_TIME_SECONDS}s) ==="
    RUST_BACKTRACE=full cargo +nightly fuzz run "$target" -- -max_total_time="$FUZZ_TIME_SECONDS"
done

echo "=== All fuzz targets completed ==="
