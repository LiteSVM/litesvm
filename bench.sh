#!/usr/bin/bash
ROOT=$(git rev-parse --show-toplevel)

cd $ROOT/crates/litesvm/test_programs
cargo build-sbf --workspace --sbf-out-dir target/deploy

cd $ROOT
RUST_LOG= cargo bench --features internal-test
