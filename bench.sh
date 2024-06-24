#!/usr/bin/bash
ROOT=$(git rev-parse --show-toplevel)

cd $ROOT/svm/test_programs
cargo build-sbf

cd $ROOT
RUST_LOG= cargo bench --features internal-test
