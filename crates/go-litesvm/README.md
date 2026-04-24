<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---

# go-litesvm

Go bindings for [LiteSVM](https://github.com/LiteSVM/litesvm), an in-process
Solana VM for fast and ergonomic program testing. `go-litesvm` exposes the
LiteSVM Rust crate through a thin C ABI and calls into it from Go via cgo.

For a typical Solana testing workflow, LiteSVM offers an experience superior
to `solana-test-validator` (slow, heavyweight) and `solana-program-test`
(reasonably fast but with rough edges). `go-litesvm` brings that same
experience to Go code.

Core Solana types (`PublicKey`, `Hash`, `Signature`) are re-used from the
[solana-foundation/solana-go](https://github.com/gagliardetto/solana-go)
SDK so values flow naturally between `go-litesvm` and the rest of the Go
Solana ecosystem.

> Status: experimental. The Go module is not yet published; today it lives
> in-tree alongside the Rust crate. The API surface is stable enough to
> write real tests against but may grow as it catches up with the Rust
> crate.

## Requirements

- Go 1.24 or newer
- Rust toolchain (to build the native C shim)
- macOS or Linux

## Build the native library

`go-litesvm` is a cgo package: before running Go code, build the Rust
`cdylib` / `staticlib` that ships with this crate.

```sh
# Debug build (fast to compile, slow to run)
cargo build -p litesvm-go

# Release build (recommended for benchmarks or CI)
cargo build -p litesvm-go --release
```

Artifacts land in `target/{debug,release}/liblitesvm_go.{a,dylib,so}`.
The cgo directives in `litesvm.go` default to `target/debug`. For release
builds, either edit the `#cgo LDFLAGS` line or override it at build time:

```sh
CGO_LDFLAGS="-L$(pwd)/target/release -llitesvm_go" go test ./...
```

## Run the tests

The shared library must be discoverable at run time.

macOS (Apple Silicon or Intel):

```sh
cd crates/go-litesvm
DYLD_LIBRARY_PATH=../../target/debug go test -v ./...
```

Linux:

```sh
cd crates/go-litesvm
LD_LIBRARY_PATH=../../target/debug go test -v ./...
```

To avoid the env var, statically link against `liblitesvm_go.a` by editing
the `#cgo LDFLAGS` line to reference the archive directly.

## Quick start

Transfer lamports between two wallets using `go-litesvm` for execution and
`solana-go` for transaction construction.

```go
package mytest

import (
    "testing"

    litesvm "github.com/LiteSVM/litesvm/crates/go-litesvm"
    solana "github.com/gagliardetto/solana-go"
    "github.com/gagliardetto/solana-go/programs/system"
)

func TestTransfer(t *testing.T) {
    svm, err := litesvm.New()
    if err != nil {
        t.Fatal(err)
    }
    defer svm.Close()

    // Fund a payer.
    priv, _ := solana.NewRandomPrivateKey()
    payer := priv.PublicKey()
    recipient := solana.NewWallet().PublicKey()

    if err := svm.Airdrop(payer, 2_000_000_000); err != nil {
        t.Fatal(err)
    }

    // Build, sign, and encode a legacy transfer with solana-go.
    blockhash, _ := svm.LatestBlockhash()
    ix := system.NewTransferInstruction(1_000_000_000, payer, recipient).Build()
    tx, _ := solana.NewTransaction(
        []solana.Instruction{ix},
        blockhash,
        solana.TransactionPayer(payer),
    )
    tx.Sign(func(k solana.PublicKey) *solana.PrivateKey {
        if k.Equals(payer) {
            return &priv
        }
        return nil
    })
    txBytes, _ := tx.MarshalBinary()

    // Execute.
    out, err := svm.SendLegacyTransaction(txBytes)
    if err != nil {
        t.Fatal(err)
    }
    defer out.Close()

    if !out.IsOk() {
        t.Fatalf("tx failed: %s\nlogs: %v", out.Error(), out.Logs())
    }

    // Inspect resulting balances.
    if lamports, _, _ := svm.Balance(recipient); lamports != 1_000_000_000 {
        t.Fatalf("recipient balance = %d, want 1_000_000_000", lamports)
    }
}
```

By default a fresh `LiteSVM` ships with the core Solana programs (System
Program, SPL Token, etc.) already loaded.

## Sending and simulating transactions

Both legacy and versioned transactions are supported. The methods accept
the bincode-encoded bytes produced by `(*solana.Transaction).MarshalBinary`.

```go
out, err := svm.SendLegacyTransaction(txBytes)     // commit on success
out, err := svm.SendVersionedTransaction(txBytes)  // same, for v0 messages

sim, err := svm.SimulateLegacyTransaction(txBytes)    // never commits
sim, err := svm.SimulateVersionedTransaction(txBytes) // same, for v0 messages
```

Every entry point returns a `*TxOutcome`. The same handle carries metadata
whether the transaction succeeded or failed, so call `IsOk` first:

```go
out, _ := svm.SendLegacyTransaction(txBytes)
defer out.Close()

if !out.IsOk() {
    t.Fatalf("error: %s", out.Error())
}

_ = out.Signature()      // solana.Signature
_ = out.ComputeUnits()   // uint64
_ = out.Fee()            // uint64
_ = out.Logs()           // []string
_ = out.InnerInstructions()

// Programs that call set_return_data expose it here.
if pid, data, ok := out.ReturnData(); ok {
    _ = pid
    _ = data
}

// Simulate also exposes the would-be post-execution account state.
for _, p := range sim.PostAccounts() {
    defer p.Account.Close()
    _ = p.Address
    _ = p.Account.Lamports()
}
```

Look up historical transactions by signature:

```go
prior := svm.GetTransaction(signature) // nil if unknown
if prior != nil {
    defer prior.Close()
}
```

## Accounts

Accounts are opaque handles. Always `Close` them (or rely on the finalizer).

```go
// Read
if acct := svm.GetAccount(addr); acct != nil {
    defer acct.Close()
    _ = acct.Lamports()
    _ = acct.Owner()
    _ = acct.Executable()
    _ = acct.Data() // copy
}

// Write
rent, _ := svm.MinimumBalanceForRentExemption(len(payload))
acct, _ := litesvm.NewAccount(rent, payload, ownerProgram, false, 0)
defer acct.Close()
_ = svm.SetAccount(targetAddr, acct)
```

## Loading programs

Load any compiled SBF program so transactions can invoke it.

```go
// From a .so file on disk.
_ = svm.AddProgramFromFile(programID, "./target/deploy/my_program.so")

// From bytes you already have in memory.
_ = svm.AddProgram(programID, bytes)

// Under a specific loader (advanced).
_ = svm.AddProgramWithLoader(programID, bytes, loaderID)
```

## Time travel and sysvars

Forward the internal clock:

```go
_ = svm.WarpToSlot(10_000_000)
```

Read and overwrite sysvars directly:

```go
c, _ := svm.Clock()
c.UnixTimestamp += 3600
_ = svm.SetClock(c)

r, _ := svm.Rent()
_ = svm.SetRent(r)

es, _ := svm.EpochSchedule()
_ = svm.SetEpochSchedule(es)

er, _ := svm.EpochRewards()
_ = svm.SetEpochRewards(er)

slot, _ := svm.LastRestartSlot()
_ = svm.SetLastRestartSlot(slot)

hashes, _ := svm.SlotHashes()
_ = svm.SetSlotHashes(hashes)

hist, _ := svm.StakeHistory()
_ = svm.SetStakeHistory(hist)
```

`SlotHistory` is a ~128 KB bitvec and uses a handle rather than a slice:

```go
sh := litesvm.NewSlotHistory()
defer sh.Close()
sh.Add(42)
_ = sh.Check(42) // SlotHistoryFound / SlotHistoryNotFound / SlotHistoryTooOld
_ = svm.SetSlotHistory(sh)
```

## Compute budget

```go
budget, set, _ := svm.ComputeBudget()
if !set {
    budget.ComputeUnitLimit = 1_400_000
}
_ = svm.SetComputeBudget(budget)
```

`ComputeBudget` is a plain struct mirroring the 44 fields of the Rust
`ComputeBudget`. `usize` fields are normalized to `uint64`; `HeapSize`
stays `uint32`.

## Feature gating

```go
// Start with all features off, then activate what you care about.
fs := litesvm.NewFeatureSet()
defer fs.Close()
_ = fs.Activate(featureID, 0)

// Or start from "everything enabled" and flip specific features off.
fs = litesvm.NewFeatureSetAllEnabled()
defer fs.Close()
_ = fs.Deactivate(featureID)

_ = svm.SetFeatureSet(fs)
```

Inspecting a feature set: `IsActive`, `ActivatedSlot`, `ActiveCount`,
`InactiveCount`, `ActiveFeatures`, `InactiveFeatures`.

## Configuration

Each setter mirrors the builder method on the Rust `LiteSVM` type:

```go
_ = svm.SetSigverify(false)            // accept unsigned / badly-signed txs
_ = svm.SetBlockhashCheck(false)       // skip recent-blockhash enforcement
_ = svm.SetTransactionHistory(0)       // 0 disables dedup; any N caps history
_ = svm.SetLogBytesLimit(-1)           // negative = unlimited
_ = svm.SetLamports(1 << 40)           // default lamports for new accounts
_ = svm.SetSysvars()                   // reset sysvars to defaults
_ = svm.SetBuiltins()                  // reload built-in programs
_ = svm.SetDefaultPrograms()           // reload SPL Token, Memo, etc.
_ = svm.SetPrecompiles()               // enable ed25519/secp256k1 precompiles
_ = svm.WithNativeMints()              // seed wrapped-SOL mint
```

## API surface

- Lifecycle: `New`, `Close`, `Version`
- Funding and balances: `Airdrop`, `Balance`, `MinimumBalanceForRentExemption`
- Blockhash: `LatestBlockhash`, `ExpireBlockhash`
- Transactions: `SendLegacyTransaction`, `SendVersionedTransaction`,
  `SimulateLegacyTransaction`, `SimulateVersionedTransaction`,
  `GetTransaction`
- `TxOutcome`: `IsOk`, `Signature`, `ComputeUnits`, `Fee`, `Logs`, `Error`,
  `ReturnData`, `InnerInstructions`, `PostAccounts`
- Accounts: `GetAccount`, `SetAccount`, `NewAccount` (`Lamports`, `Owner`,
  `Executable`, `RentEpoch`, `Data`)
- Programs: `AddProgram`, `AddProgramFromFile`, `AddProgramWithLoader`
- Time: `WarpToSlot`
- Sysvars: `Clock`, `Rent`, `EpochSchedule`, `EpochRewards`,
  `LastRestartSlot`, `SlotHashes`, `StakeHistory`, `SlotHistory`
- Compute budget: `ComputeBudget`, `SetComputeBudget`
- Feature set: `NewFeatureSet`, `NewFeatureSetAllEnabled`, `IsActive`,
  `ActivatedSlot`, `Activate`, `Deactivate`, `ActiveCount`, `InactiveCount`,
  `ActiveFeatures`, `InactiveFeatures`, `SetFeatureSet`
- Configuration: `SetSigverify`, `Sigverify`, `SetBlockhashCheck`,
  `SetTransactionHistory`, `SetLogBytesLimit`, `SetLamports`, `SetSysvars`,
  `SetBuiltins`, `SetDefaultPrograms`, `SetPrecompiles`, `WithNativeMints`

## Thread safety

`LiteSVM` handles are not safe for concurrent use from multiple goroutines.
The underlying Rust type is not `Sync`, and most method calls mutate
internal state. Either confine a handle to a single goroutine or wrap it
in a `sync.Mutex`.

## Panic safety

Every Rust `extern "C"` entry point wraps its body in
`std::panic::catch_unwind`. Panics are converted to a non-zero return code
plus a descriptive error string; they are never unwound across the C
boundary.

## Repository layout

```
crates/go-litesvm/
  Cargo.toml          Rust cdylib + staticlib
  src/lib.rs          C ABI implementation
  include/litesvm.h   Hand-written C header, kept in sync with lib.rs
  go.mod
  litesvm.go          Idiomatic Go wrapper
  litesvm_test.go     End-to-end tests
```

The Rust crate and Go module share a single directory. Go ignores
`Cargo.toml` / `src/`; Cargo ignores `go.mod` / `*.go`.

## Extending

To wire a new LiteSVM method through to Go, follow the pattern already
used throughout `src/lib.rs` and `litesvm.go`:

1. Add an `extern "C" fn` in `src/lib.rs`, wrapped in `guard(...)`, using
   the `handle_ref` / `handle_mut` / `pubkey_from_ptr` helpers. Set
   thread-local error strings on failure.
2. Declare the function in `include/litesvm.h`.
3. Add the Go method in `litesvm.go`. Translate non-zero return codes via
   `lastError`.
4. Add a test to `litesvm_test.go`.

For result-carrying operations, prefer extending the existing `TxOutcome`
shape rather than reproducing Rust `Result` / enum discriminants across
the FFI.

## License

See the root of the repository.
