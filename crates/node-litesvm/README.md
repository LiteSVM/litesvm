<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---
# LiteSVM (NodeJS)

This is the NodeJS wrapper for [LiteSVM](https://github.com/LiteSVM/litesvm). It brings best-in-class Solana testing
to NodeJS, giving you a powerful, fast and ergonomic way to test Solana programs in TS/JS.

For a standard testing workflow, LiteSVM offers an experience superior to `solana-test-validator` (slow, unwieldy)
and `bankrun` (reasonably fast and powerful, but inherits a lot of warts from `solana-program-test`).

## Minimal example (solana/web3.js)

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

### Minimal Example (solana/kit)

Below is an example that demonstrates the usage of Solana Kit:

```typescript
import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { generateKeyPairSigner } from "@solana/signers";
import type { Blockhash, Address } from "@solana/kit";
import {
  appendTransactionMessageInstructions,
  createTransactionMessage,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import {
  SYSTEM_PROGRAM_ADDRESS,
  getTransferSolInstruction,
} from "@solana-program/system";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("one transfer", async () => {
  const svm = new LiteSVMKit();

  // Alice (fee payer & source)
  const alice = await generateKeyPairSigner();
  svm.airdrop(alice.address, LAMPORTS_PER_SOL);

  // Bob (destination) — create a system-owned account for him
  const bob = await generateKeyPairSigner();
  const bobAddress: Address = bob.address;
  svm.setAccount(bobAddress, {
    address: bobAddress,
    owner: SYSTEM_PROGRAM_ADDRESS,
    executable: false,
    lamports: 0n,
    data: new Uint8Array(),
    rentEpoch: 0n,
  });

  const transferLamports = 1_000_000n;
  const blockhash = svm.latestBlockhash() as Blockhash;

  // Proper System Program transfer instruction (Kit-native)
  const transferIx = getTransferSolInstruction({
    source: alice,
    destination: bobAddress,
    amount: transferLamports,
  });

  // Build and send the transaction
  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    (msg) => setTransactionMessageFeePayerSigner(alice, msg),
    (msg) =>
      setTransactionMessageLifetimeUsingBlockhash(
        { blockhash, lastValidBlockHeight: 0n },
        msg
      ),
    (msg) => appendTransactionMessageInstructions([transferIx], msg)
  );

  await svm.sendTransaction(tx);

  const bobBalance = svm.getBalance(bobAddress);
  assert.strictEqual(bobBalance, transferLamports);
});

```
Note: by default the `LiteSVM` instance includes some core programs such as
the System Program and SPL Token.

## Installation

```
yarn add litesvm
```

## Contributing

Make sure you have Yarn and the Rust toolchain installed.

Then run `yarn` to install deps, run `yarn build` to build the binary and `yarn test` to run the tests.
