#!/bin/bash

set -ex

cargo install cargo-fuzz

RUST_BACKTRACE=full cargo  +nightly fuzz run fuzz_value -- -max_total_time=20m