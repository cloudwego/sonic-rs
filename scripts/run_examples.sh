#!/bin/bash

set -xe

examples=$(cargo build --example 2>&1 | grep -v ":")

for example in $examples; do
    echo "Running example $example"
    cargo run --example $example
done

