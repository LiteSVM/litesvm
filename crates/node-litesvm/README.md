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
import { LiteSVM } from "litesvm";
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
import { expect, it } from "vitest";

it("transfers SOL from one wallet to another", async () => {
	// Given a payer with 2 SOL and a recipient with 0 SOL.
	const svm = new LiteSVM();
	const payer = await generateKeyPairSigner();
	const recipient = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(2_000_000_000n));

	// When we send 1 SOL from the payer to the recipient.
	const lifetime = svm.latestBlockhashLifetime();
	const instruction = getTransferSolInstruction({
		source: payer,
		destination: recipient.address,
		amount: lamports(1_000_000_000n),
	});
	const transaction = await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash(lifetime, tx),
		(tx) => appendTransactionMessageInstruction(instruction, tx),
		(tx) => signTransactionMessageWithSigners(tx),
	);
	svm.sendTransaction(transaction);

	// Then we expect the accounts to have the correct balances.
	expect(svm.getBalance(recipient.address)).toBe(lamports(1_000_000_000n));
	expect(svm.getBalance(payer.address)).toBeLessThan(lamports(1_000_000_000n));
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
