#!/bin/bash

set -ex

cargo test

cargo test --features arbitrary_precision

cargo test --features sort_keys

cargo test --features use_raw

cargo test --features utf8_lossy

examples=$(cargo build --example 2>&1 | grep -v ":")

for example in $examples; do
    echo "Running example $example"
    cargo run --example $example
done




