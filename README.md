<div align="center">
<h1 align="center">
<img src="https://raw.githubusercontent.com/PKief/vscode-material-icon-theme/ec559a9f6bfd399b82bb44393651661b08aaf7ba/icons/folder-markdown-open.svg" width="100" />
<br></h1>
<h3>LiteSVM</h3>
</div>

---

## 📍 Overview

`litesvm` is a fast and lightweight Solana VM simulator for testing programs. It takes inspiration from the good parts of the [`solana-program-test`](https://github.com/solana-labs/solana/tree/master/program-test) crate, while offering superior performance and developer experience.
---

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

#[test]
fn system_transfer() {
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
    let tx_res = svm.send_transaction(tx);

    let from_account = svm.get_account(&from);
    let to_account = svm.get_account(&to);

    assert!(tx_res.is_ok());
    assert_eq!(from_account.unwrap().lamports, 4936);
    assert_eq!(to_account.unwrap().lamports, 64);
}
```
