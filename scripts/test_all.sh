#!/bin/bash

set -ex

cargo test

./scripts/run_examples.sh

./scripts/fuzz.sh



