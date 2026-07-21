# LiteSVM for Web3.js

`litesvm-web3js` provides a Web3.js v3 interface for
[LiteSVM](https://github.com/LiteSVM/litesvm). It uses `litesvm-core` for native
execution and `@solana/web3.js@3.0.0-rc.2` for addresses, accounts, and
transactions.

```sh
yarn add litesvm-web3js @solana/web3.js@3.0.0-rc.2
```

Use the `litesvm` package instead when building transactions with
`@solana/kit`.

## Example

```ts
import {
	FailedTransactionMetadata,
	LiteSVM,
} from "litesvm-web3js";
import {
	Keypair,
	LAMPORTS_PER_SOL,
	SystemProgram,
	Transaction,
} from "@solana/web3.js";

const svm = new LiteSVM();
const payer = await Keypair.generate();
const recipient = await Keypair.generate();

svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));

const transaction = new Transaction({
	feePayer: payer.publicKey,
	blockhash: svm.latestBlockhash(),
	lastValidBlockHeight: 0n,
}).add(
	SystemProgram.transfer({
		fromPubkey: payer.publicKey,
		toPubkey: recipient.publicKey,
		lamports: 1_000_000n,
	}),
);

await transaction.sign(payer);
const result = await svm.sendTransaction(transaction);
if (result instanceof FailedTransactionMetadata) {
	throw new Error(`Transaction failed: ${result.err()}`);
}
```

Web3.js v3 signing and legacy transaction serialization are asynchronous, so
`sendTransaction` and `simulateTransaction` return promises in this package.
Account data uses `Uint8Array`; lamports, rent epochs, account space, slots, and
block heights use `bigint`.
