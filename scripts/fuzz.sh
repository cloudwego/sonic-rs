#!/bin/bash

set -ex

cargo install cargo-fuzz

cargo +nightly fuzz run fuzz_value -- -max_total_time=5m