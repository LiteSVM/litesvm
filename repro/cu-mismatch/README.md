# CU Mismatch Repro

This directory contains a tracked CU mismatch repro between:
- RPC `simulateTransaction` CU
- local `LiteSVM::simulate_transaction` CU

Canonical saved snapshot:
- `repro/cu-mismatch/snapshots/k13_min_2_3`
- subset is top-level instruction keep-set `[2,3]` from original `k13` transaction

## Offline Repro (No RPC)

```bash
repro/cu-mismatch/run_saved_repro.sh
```

Direct `cargo` command (also offline):

```bash
cargo run --release \
  --manifest-path repro/cu-mismatch/Cargo.toml \
  --bin cu_mismatch_repro \
  -- repro/cu-mismatch/snapshots/k13_min_2_3
```

`cu_mismatch_repro` only reads saved files from the snapshot directory:
- `tx.json`, `tx_json.json`, `simulate.json`
- `accounts.json`, `account_keys.txt`
- `programs/*.so`

## Offline Engine Comparison (LiteSVM vs Mollusk)

```bash
repro/cu-mismatch/compare_saved_repro.sh
```

Direct `cargo` command:

```bash
RUST_LOG=error cargo run --release \
  --manifest-path repro/cu-mismatch/Cargo.toml \
  --bin compare_mollusk \
  -- repro/cu-mismatch/snapshots/k13_min_2_3
```

This prints CU deltas for:
- LiteSVM vs saved RPC simulate CU
- Mollusk vs saved RPC simulate CU
- LiteSVM vs Mollusk

Why we think loader semantics are the main mismatch driver:
- On the `k13` snapshot, loading dumped custom programs via plain `add_program`
  produced about `litesvm_cu=217055` vs `rpc_sim_units=126082` (`+90973`).
- Loading those same dumped programs as upgradeable `Program + ProgramData`
  accounts dropped LiteSVM to about `litesvm_cu=130976` (delta about `+4894`).
- That points to executable account shape/loader path as the primary cause.

## Capturing New Snapshots (Online, One-Time Data Collection)

```bash
repro/cu-mismatch/capture_snapshot.sh <signature> [rpc_url] [snapshot_dir]
```

If `snapshot_dir` is omitted, capture defaults to:
- `repro/cu-mismatch/snapshots/<signature>`

## Output

The repro prints:
- `onchain_cu`
- `rpc_sim_units`, `rpc_sim_err`
- `litesvm_cu`, `litesvm_err`
- `delta_litesvm_minus_rpc`
- `delta_litesvm_minus_onchain`

If `rpc_sim_err != null`, the sample is not usable for apples-to-apples CU comparison.

This repro path is fully offline at runtime; no RPC calls are required to run
`run_saved_repro.sh` or `cu_mismatch_repro`.
