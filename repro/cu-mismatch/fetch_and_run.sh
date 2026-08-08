#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 3 ]]; then
  echo "usage: $0 <signature> [rpc_url] [snapshot_dir]"
  exit 1
fi

SIG="$1"
RPC_URL="${2:-https://api.mainnet-beta.solana.com}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
SNAPSHOT_DIR="${3:-$SCRIPT_DIR/snapshots/$SIG}"

"$SCRIPT_DIR/capture_snapshot.sh" "$SIG" "$RPC_URL" "$SNAPSHOT_DIR"

cargo run --release \
  --manifest-path "$REPO_ROOT/repro/cu-mismatch/Cargo.toml" \
  --bin cu_mismatch_repro \
  -- "$SNAPSHOT_DIR"
