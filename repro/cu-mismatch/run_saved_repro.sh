#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
SNAPSHOT_DIR="${1:-$SCRIPT_DIR/snapshots/k13_min_2_3}"

cargo run --release \
  --manifest-path "$REPO_ROOT/repro/cu-mismatch/Cargo.toml" \
  --bin cu_mismatch_repro \
  -- "$SNAPSHOT_DIR"
