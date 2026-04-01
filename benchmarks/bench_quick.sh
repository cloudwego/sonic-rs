#!/bin/bash
# Quick benchmark for iterative optimization.
# Usage: ./bench_quick.sh [label]
LABEL=${1:-"current"}
cargo bench --bench deserialize_value -- '(golang_source|citm_catalog|lottie)/sonic_rs_dom::from_slice$' 2>&1 \
  | grep 'time:' | grep -v estimated \
  | awk -v label="$LABEL" '
    NR==1 {printf "%-20s citm_catalog: %s\n", label, $0}
    NR==2 {printf "%-20s golang_source: %s\n", label, $0}
    NR==3 {printf "%-20s lottie: %s\n", label, $0}
  '
