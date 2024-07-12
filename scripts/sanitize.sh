#!/bin/bash

set -ex

export ASAN_OPTIONS="disable_coredump=0:unmap_shadow_on_exit=1:abort_on_error=1"

testcase_lists() {
    cargo test -- -Zunstable-options --list --format json | jq -c 'select(.type=="test") | .name' | awk -F'"' '{print $2}' | awk '{print ($2) ? $3 : $1}'
    return $?
}

sanitize() {
    SAN=$1
    TARGET=$2
    TESTCASE=$3
    echo "Running tests with $SAN on $TARGET"
    # # use single thread to make error info more readable and accurate
    RUSTFLAGS="-Zsanitizer=$SAN" RUSTDOCFLAGS="-Zsanitizer=$SAN" cargo test --target $TARGET $3 -- --test-threads=1

    RUSTFLAGS="-Zsanitizer=$SAN" RUSTDOCFLAGS="-Zsanitizer=$SAN" cargo test --doc --package sonic-rs --target $TARGET $3  -- --show-output --test-threads=1
}

sanitize_single() {
    SAN=$1
    TARGET=$2
    for CASE in $(testcase_lists); do
        sanitize $SAN $TARGET $CASE
    done
}

for san in address leak; do
    echo "Running tests with $san"
    # sanitize $san "x86_64-unknown-linux-gnu"
    sanitize_single $san "x86_64-unknown-linux-gnu"
done


