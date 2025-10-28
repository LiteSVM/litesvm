import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, FailedTransactionMetadata } from "litesvm";
import {
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	blockhash,
	lamports,
} from "@solana/kit";
import { TransactionErrorFieldless } from "internal";

test("non-existent program", async () => {
	const svm = new LiteSVM();
	const programKeypair = await generateKeyPairSigner();
	const programId = programKeypair.address;
	const payer = await generateKeyPairSigner();
	const LAMPORTS_PER_SOL = lamports(1_000_000_000n);
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const instruction = {
		programAddress: programId,
		accounts: [] as Array<{ address: any; role: number }>,
		data: new Uint8Array(1),
	};
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 1000n }, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	const res = svm.sendTransaction(signedTransaction);
	if (res instanceof FailedTransactionMetadata) {
		const err = res.err();
		assert.strictEqual(
			err,
			TransactionErrorFieldless.InvalidProgramForExecution,
		);
	} else {
		throw new Error("Expected transaction failure");
	}
});