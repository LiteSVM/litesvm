# SodaSVM

## 📍 Overview

SodaSVM is a privacy-focused, fee-free Solana Virtual Machine that addresses the scalability limitations of transaction fees while providing strong privacy guarantees and censorship resistance. Built on a high-performance SVM foundation, SodaSVM enables zero-cost internal transfers with optional privacy features.

## 🚀 Key Features

- **Zero Transaction Fees**: Internal transfers cost nothing
- **Solana Compatible**: Full ecosystem compatibility

## 🤖 Quick Start

### Installation

```sh
cargo add sodasvm
```

### Basic Usage

```rust
use sodasvm::SodaSVM;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

let mut svm = SodaSVM::new();
let from_keypair = Keypair::new();
let to_keypair = Keypair::new();

// Airdrop some funds
svm.airdrop(&from_keypair.pubkey(), 1_000_000).unwrap();

// Fee-free transfer
let instruction = transfer(&from_keypair.pubkey(), &to_keypair.pubkey(), 100_000);
let message = Message::new(&[instruction], Some(&from_keypair.pubkey()));
let tx = Transaction::new(&[&from_keypair], message, svm.latest_blockhash());

// Execute with zero fees
let result = svm.send_transaction_free(tx).unwrap();
```

## 🛠️ Development

### Run Tests

```sh
# Build test programs first
cd crates/litesvm/test_programs && cargo build-sbf

# Run all tests
cargo test
```

### Test SodaSVM

```sh
# Test the SodaSVM crate specifically
cargo test -p sodasvm
```


## 🔐 Security

SodaSVM prioritizes security through:
- Cryptographic privacy guarantees
- Economic incentive mechanisms
- Multi-signature operations
- Regular security audits


## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](./CONTRIBUTING.md) for details.

## 📄 License

Licensed under the Apache License, Version 2.0. See [LICENSE](./LICENSE) for details.

