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

This Kit example just transfers lamports from Alice to Bob without loading
any programs of our own. It uses the [Node.js test runner](https://nodejs.org/api/test.html).

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { FailedTransactionMetadata, LiteSVM } from "litesvm/kit";
import { getTransferSolInstruction } from "@solana-program/system";
import {
	appendTransactionMessageInstruction,
	createTransactionMessage,
	generateKeyPairSigner,
	lamports,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";

test("it transfers SOL from one wallet to another", async () => {
	// Given a payer with 2 SOL and a recipient with 0 SOL.
	const svm = new LiteSVM();
	const payer = await generateKeyPairSigner();
	const recipient = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(2_000_000_000n));

	// When we send 1 SOL from the payer to the recipient.
	const instruction = getTransferSolInstruction({
		source: payer,
		destination: recipient.address,
		amount: lamports(1_000_000_000n),
	});
	const transaction = await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => svm.setTransactionMessageLifetimeUsingLatestBlockhash(tx),
		(tx) => appendTransactionMessageInstruction(instruction, tx),
		(tx) => signTransactionMessageWithSigners(tx),
	);

	const result = svm.sendTransaction(transaction);
	if (result instanceof FailedTransactionMetadata) {
		throw new Error(`Transaction failed: ${result.err()}`);
	}

	// Then we expect the accounts to have the correct balances.
  const payerBalance = svm.getBalance(payer.address);

  assert.strictEqual(
		svm.getBalance(recipient.address),
    lamports(1_000_000_000n),
  );
  assert(payerBalance !== null);
  assert(payerBalance < lamports(1_000_000_000n));
});
```

web3.js users can use the compatibility entrypoint:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { FailedTransactionMetadata, LiteSVM } from "litesvm/web3js";
import {
	Keypair,
	LAMPORTS_PER_SOL,
	SystemProgram,
	Transaction,
} from "@solana/web3.js";

test("it transfers SOL with web3.js", async () => {
	const svm = new LiteSVM();
	const payer = await Keypair.generate();
	const recipient = await Keypair.generate();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));

	const tx = new Transaction({
		feePayer: payer.publicKey,
		blockhash: svm.latestBlockhash(),
		lastValidBlockHeight: 0n,
	}).add(
		SystemProgram.transfer({
			fromPubkey: payer.publicKey,
			toPubkey: recipient.publicKey,
			lamports: 1_000_000_000n,
		}),
	);
	await tx.sign(payer);

	const result = await svm.sendTransaction(tx);
	if (result instanceof FailedTransactionMetadata) {
		throw new Error(`Transaction failed: ${result.err()}`);
	}

	assert.strictEqual(svm.getBalance(recipient.publicKey), 1_000_000_000n);
});
```

Root imports (`import { LiteSVM } from "litesvm"`) point to the Kit-compatible API. Prefer `litesvm/kit`
or `litesvm/web3js` in new code so the Solana SDK surface is explicit.

Note: by default the `LiteSVM` instance includes some core programs such as
the System Program and SPL Token.

## Installation

```
yarn add litesvm
```

## Contributing

Make sure you have Yarn and the Rust toolchain installed.

Then run `yarn` to install deps, run `yarn build` to build the binary and `yarn test` to run the tests.
