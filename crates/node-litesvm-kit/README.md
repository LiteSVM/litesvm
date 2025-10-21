<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---
# LiteSVMKit (NodeJS)

This is the NodeJS wrapper for [LiteSVM](https://github.com/LiteSVM/litesvm). It brings best-in-class Solana testing
to NodeJS, giving you a powerful, fast and ergonomic way to test Solana programs in TS/JS.

For a standard testing workflow, LiteSVMKit offers an experience superior to `solana-test-validator` (slow, unwieldy)
and `bankrun` (reasonably fast and powerful, but inherits a lot of warts from `solana-program-test`).

## Minimal example

This example just transfers lamports from Alice to Bob without loading
any programs of our own. It uses the [Node.js test runner](https://nodejs.org/api/test.html).

```ts
mport { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import {
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	blockhash,
} from "@solana/kit";
import { getTransferSolInstruction } from "@solana-program/system";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("one transfer", async () => {
	const svm = new LiteSVMKit();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	const receiver = await generateKeyPairSigner();
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const transferLamports = 1_000_000n;
	
	const transferInstruction = getTransferSolInstruction({
		source: payer,
		destination: receiver.address,
		amount: transferLamports,
	});
	
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([transferInstruction], tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	
	svm.sendTransaction(signedTransaction);
	const balanceAfter = svm.getBalance(receiver.address);
	assert.strictEqual(balanceAfter, transferLamports);
});
```
Note: by default the `LiteSVMKit` instance includes some core programs such as
the System Program and SPL Token.

## Installation

```
yarn add litesvm
```

## Contributing

Make sure you have Yarn and the Rust toolchain installed.

Then run `yarn` to install deps, run `yarn build` to build the binary and `yarn test` to run the tests.
