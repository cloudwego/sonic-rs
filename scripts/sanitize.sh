#!/bin/bash

set -ex

export ASAN_OPTIONS="disable_coredump=0:unmap_shadow_on_exit=1:abort_on_error=1"

testcase_lists() {
    cargo test -- -Zunstable-options --list --format json
    local result=$?
    if [ ${result} -ne 0 ]; then
        exit -1
    fi
    cargo test -- -Zunstable-options --list --format json  | jq -c 'select(.type=="test") | .name' | awk -F'"' '{print $2}' | awk '{print ($2) ? $3 : $1}'
    return $?
}

sanitize() {
    local san="$1"
    local target="$2"
    local testcase="$3"
    # use single thread to make error info more readable and accurate
    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo test --target ${target} ${testcase} -- --test-threads=1
    RUSTFLAGS="-Zsanitizer=${san}" RUSTDOCFLAGS="-Zsanitizer=${san}" cargo test --doc --package sonic-rs --target ${target} ${testcase}  -- --show-output --test-threads=1
}

sanitize_single() {
    local san="$1"
    local target="$2"
    local lists=$(testcase_lists)
    for case in ${lists}; do
        sanitize ${san} ${target} ${case}
    done
}

main() {
    for san in address leak; do
        echo "Running tests with $san"
        sanitize_single $san "x86_64-unknown-linux-gnu"
    done
}

main "$@"


