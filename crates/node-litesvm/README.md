<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---
# LiteSVM (NodeJS)

This is the NodeJS wrapper for [LiteSVM](https://github.com/LiteSVM/litesvm). It brings best-in-class Solana testing
to NodeJS, giving you a powerful, fast and ergonomic way to test Solana programs in TS/JS.

For a standard testing workflow, LiteSVM offers an experience superior to `solana-test-validator` (slow, unwieldy)
and `bankrun` (reasonably fast and powerful, but inherits a lot of warts from `solana-program-test`).

## Minimal example

This example just transfers lamports from Alice to Bob without loading
any programs of our own. It uses the [Node.js test runner](https://nodejs.org/api/test.html).

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import {
	PublicKey,
	Transaction,
	SystemProgram,
	Keypair,
	LAMPORTS_PER_SOL,
} from "@solana/web3.js";

test("one transfer", () => {
	const svm = new LiteSVM();
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const receiver = PublicKey.unique();
	const blockhash = svm.latestBlockhash();
	const transferLamports = 1_000_000n;
	const ixs = [
		SystemProgram.transfer({
			fromPubkey: payer.publicKey,
			toPubkey: receiver,
			lamports: transferLamports,
		}),
	];
	const tx = new Transaction();
	tx.recentBlockhash = blockhash;
	tx.add(...ixs);
	tx.sign(payer);
	svm.sendTransaction(tx);
	const balanceAfter = svm.getBalance(receiver);
	assert.strictEqual(balanceAfter, transferLamports);
});
```
Note: by default the `LiteSVM` instance includes some core programs such as
the System Program and SPL Token.

## @solana/kit Integration

LiteSVM now supports [@solana/kit](https://github.com/solana-labs/solana-kit) types and methods alongside the traditional @solana/web3.js API. This provides a more modern, type-safe experience for Solana development.

### Kit Usage Example

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKitClass } from "litesvm";
import { address } from "@solana/kit";

test("kit integration", () => {
	const svm = new LiteSVMKitClass();
	
	// Use Kit Address types
	const testAddress = address("11111111111111111111111111111111");
	
	// Get account info using Kit methods
	const account = svm.getAccountKit(testAddress);
	assert(account !== null);
	assert.strictEqual(account.executable, true);
	
	// Get balance using Kit methods  
	const balance = svm.getBalanceKit(testAddress);
	assert(balance !== null);
	assert(balance > 0n);
});
```

### Kit API Methods

The `LiteSVMKitClass` provides Kit-compatible versions of all major LiteSVM methods:

- **Account Management:**
  - `getAccountKit(address: Address): KitAccountInfo | null`
  - `setAccountKit(address: Address, account: KitAccountInfo): void`
  - `getBalanceKit(address: Address): bigint | null`

- **Configuration:** All standard LiteSVM configuration methods (withSysvars, withDefaultPrograms, etc.)

- **Types:** Full support for Kit types including `Address`, `KitAccountInfo`, and more

### Migration from web3.js

You can gradually migrate from @solana/web3.js to @solana/kit by using both APIs side-by-side:

```ts
import { LiteSVMKitClass } from "litesvm";
import { PublicKey } from "@solana/web3.js";
import { address } from "@solana/kit";

const svm = new LiteSVMKitClass();

// Use web3.js PublicKey
const pubkey = new PublicKey("11111111111111111111111111111111");
const web3Account = svm.getAccount(pubkey);

// Use Kit Address  
const addr = address("11111111111111111111111111111111");
const kitAccount = svm.getAccountKit(addr);

// Both return equivalent data in their respective formats
```

## Installation

```
yarn add litesvm
```

## Contributing

Make sure you have Yarn and the Rust toolchain installed.

Then run `yarn` to install deps, run `yarn build` to build the binary and `yarn test` to run the tests.
