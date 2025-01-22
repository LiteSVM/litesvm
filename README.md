<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---

# LiteSVM

[<img alt="github" src="https://img.shields.io/badge/github-LiteSVM/litesvm-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/LiteSVM/litesvm)
[<img alt="crates.io" src="https://img.shields.io/crates/v/litesvm.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/litesvm)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-litesvm-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/litesvm/latest/litesvm/)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/LiteSVM/litesvm/CI.yml?branch=master&style=for-the-badge" height="20">](https://github.com/LiteSVM/litesvm/actions?query=branch%3Amaster)

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
use solana_program::{message::Message, pubkey::Pubkey, system_instruction::transfer};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

let from_keypair = Keypair::new();
let from = from_keypair.pubkey();
let to = Pubkey::new_unique();

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

### 🛠️ Developing litesvm

#### Run the tests

The tests in this repo use some test programs you need to build first (Solana CLI >= 1.18.8 required):

```cd crates/litesvm/test_programs && cargo build-sbf```

Then just run `cargo test`.
