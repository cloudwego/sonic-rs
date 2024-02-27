#!/bin/bash

set -ex

cargo install cargo-fuzz

cargo fuzz run fuzz_value -- -max_total_time=20m