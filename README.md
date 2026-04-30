<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---

# LiteSVM

[<img alt="github" src="https://img.shields.io/badge/github-LiteSVM/litesvm-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/LiteSVM/litesvm)
[<img alt="crates.io" src="https://img.shields.io/crates/v/litesvm.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/litesvm)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-litesvm-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/litesvm/latest/litesvm/)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/LiteSVM/litesvm/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/LiteSVM/litesvm/actions?query=branch%3Amaster)

## 📍 Overview

`litesvm` is a fast and lightweight library for testing Solana programs. It works by creating an in-process Solana VM optimized for program developers. This makes it much faster to run and compile than alternatives like `solana-program-test` and `solana-test-validator`. In a further break from tradition, it has an ergonomic API with sane defaults and extensive configurability for those who want it.

## 🚀 Getting Started

### 🔧 Installation

```sh
cargo add --dev litesvm
```

### 🤖 Minimal Example

```rust
use litesvm::LiteSVM;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

let from_keypair = Keypair::new();
let from = from_keypair.pubkey();
let to = Address::new_unique();

let mut svm = LiteSVM::new();
svm.airdrop(&from, 10_000).unwrap();

let instruction = transfer(&from, &to, 64);
let tx = Transaction::new(
    &[&from_keypair],
    Message::new(&[instruction], Some(&from)),
    svm.latest_blockhash(),
);
let tx_res = svm.send_transaction(tx).unwrap();

let from_account = svm.get_account(&from);
let to_account = svm.get_account(&to);
assert_eq!(from_account.unwrap().lamports, 4936);
assert_eq!(to_account.unwrap().lamports, 64);
```

## Capabilities

Beyond simple transfers, `litesvm` supports:

- **Program loading** — load compiled `.so` files with `add_program_from_file` or raw bytes with `add_program`. Pull programs from mainnet/devnet using `solana program dump`.
- **Simulation** — dry-run a transaction without committing state changes using `simulate_transaction`.
- **Time travel** — overwrite the `Clock` sysvar with `set_sysvar::<Clock>()`, or jump to a future slot with `warp_to_slot`.
- **Arbitrary account writes** — use `set_account` to write any account state bypassing runtime checks (e.g. give a test wallet a large USDC balance without owning the mint keypair).
- **Compute budget control** — override compute unit limits and heap size with `with_compute_budget`.
- **Transaction history** — look up past transactions by signature with `get_transaction`; configure history capacity with `with_transaction_history`.
- **Sigverify control** — disable signature checking with `with_sigverify(false)` to speed up tests that don't need signing.
- **Custom syscalls** — register custom BPF syscalls with `with_custom_syscall`.
- **Register tracing** — capture BPF register traces (requires the `register-tracing` feature flag).

## Additional Crates

### `litesvm-token`

[`litesvm-token`](https://crates.io/crates/litesvm-token) provides ergonomic helpers for testing SPL Token programs. Rather than hand-rolling the instructions for creating mints, token accounts, and ATAs, it exposes a builder-style API covering the full range of token operations: `CreateMint`, `CreateAssociatedTokenAccount`, `MintTo`, `Transfer`, `Burn`, `Approve`, and their checked variants, plus authority management (`SetAuthority`, `FreezeAccount`, `ThawAccount`).

```sh
cargo add --dev litesvm-token
```

See the [SPL token testing guide](https://www.litesvm.com/docs/additional-crates/testing-with-spl-tokens) for a full walkthrough.

### `litesvm-loader`

[`litesvm-loader`](https://crates.io/crates/litesvm-loader) provides helpers for working with Solana's upgradeable BPF loader in LiteSVM. It wraps the repetitive deployment flow for upgradeable programs by creating the buffer account, writing program bytes in chunks, deploying the program, and exposing a helper for changing the upgrade authority.

```sh
cargo add --dev litesvm-loader
```

See the [loader API docs](https://www.litesvm.com/docs/additional-crates/testing-with-litesvm-loader) for the available helpers.

### `litesvm-utils`

[`litesvm-utils`](https://crates.io/crates/litesvm-utils) dramatically reduces test boilerplate through three ergonomic traits that extend `LiteSVM`:

- **`TestHelpers`** — create funded accounts, token mints, and ATAs; derive PDAs; and manipulate slots, all in a single method call.
- **`AssertionHelpers`** — readable one-liner assertions for account existence, ownership, SOL/token balances, and account data length.
- **`TransactionHelpers`** — execute instructions and assert success, failure, or specific error codes without manually constructing transactions.

It also ships a `LiteSVMBuilder` for fluent, chainable test environment setup. The crate is framework-agnostic and works with native, Anchor, and SPL programs.

```sh
cargo add --dev litesvm-utils
```

See the [litesvm-utils testing guide](https://www.litesvm.com/docs/additional-crates/testing-with-litesvm-utils) for a full walkthrough.

### `anchor-litesvm`

[`anchor-litesvm`](https://crates.io/crates/anchor-litesvm) brings Anchor-native testing to LiteSVM with syntax mirroring `anchor-client` — but with no RPC overhead. It's the recommended way to test Anchor programs with LiteSVM.

- **`AnchorContext`** — manages the `LiteSVM` instance, payer, and program in one place. Use `AnchorLiteSVM::build_with_program()` to set up the full test environment in a single call.
- **`Program` builder** — fluent, type-safe instruction building via `accounts()`, `args()`, and `instruction()`, using the client types generated by `declare_program!` from your IDL.
- **Account deserialization** — automatically fetches and deserializes Anchor accounts (including PDAs), handling discriminators for you.
- **Event parsing** — extracts and deserializes typed events from transaction logs.

```sh
cargo add --dev anchor-litesvm
```

See the [anchor-litesvm testing guide](https://www.litesvm.com/docs/additional-crates/testing-with-anchor-litesvm) for a full walkthrough.

## Further Reading

- Full documentation: [litesvm.com](https://litesvm.com)
- Full API reference: [docs.rs/litesvm](https://docs.rs/litesvm/latest/litesvm/)
- Node.js wrapper: [`litesvm` on npm](https://www.npmjs.com/package/litesvm) — see [`crates/node-litesvm`](crates/node-litesvm) for its README and tutorial

### 🛠️ Developing litesvm

#### Run the tests

The tests in this repo use some test programs you need to build first (Solana CLI >= 1.18.8 required):

```cd crates/litesvm/test_programs && cargo build-sbf```

Then just run `cargo test`.
