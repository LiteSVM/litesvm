<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---
# LiteSVM (NodeJS)

This is the NodeJS wrapper for [LiteSVM](https://github.com/LiteSVM/litesvm). It brings best-in-class Solana testing
to NodeJS, giving you a powerful, fast and ergonomic way to test Solana programs in TS/JS.

For a standard testing workflow, LiteSVM offers an experience superior to `solana-test-validator` (slow, unwieldy)
and `bankrun` (reasonably fast and powerful, but inherits a lot of warts from `solana-program-test`).

## Minimal example (olana/web3.js)

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
// tests/kit-oneTransfer.test.ts
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
import { getTransferSolInstruction, SYSTEM_PROGRAM_ADDRESS } from "@solana-program/system";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("one transfer", async () => {
  const svm = new LiteSVMKit();

  // Alice (sender / fee payer)
  const alice = await generateKeyPairSigner();
  await svm.airdrop(alice.address, LAMPORTS_PER_SOL);

  // Bob (recipient) — make sure a System-owned account exists
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

  const transferAmount = 1_000_000n; // 0.001 SOL
  const { blockhash } = svm.latestBlockhash() as Blockhash | any; // supports either shape

  // Build a proper System transfer ix (no explicit "version" needed)
  const transferIx = getTransferSolInstruction({
    source: alice,                // you can pass the signer directly
    destination: bobAddress,
    lamports: transferAmount,
  });

  const tx = pipe(
    createTransactionMessage(), // no version arg
    (m) => setTransactionMessageFeePayerSigner(alice, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash, lastValidBlockHeight: 0n }, m),
    (m) => appendTransactionMessageInstructions([transferIx], m),
  );

  await svm.sendTransaction(tx);

  const bobBalance = svm.getBalance(bobAddress);
  assert.strictEqual(bobBalance, transferAmount);
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
