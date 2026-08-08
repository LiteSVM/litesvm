#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
SNAPSHOT_DIR="${1:-$SCRIPT_DIR/snapshots/k13_min_2_3}"

RUST_LOG="${RUST_LOG:-error}" cargo run --release \
  --manifest-path "$REPO_ROOT/repro/cu-mismatch/Cargo.toml" \
  --bin compare_mollusk \
  -- "$SNAPSHOT_DIR"
