import { test } from "node:test";
import assert from "node:assert/strict";
import { FailedTransactionMetadata } from "litesvm";
import {
	createTransactionMessage,
	appendTransactionMessageInstructions,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
	pipe,
	generateKeyPairSigner,
	lamports,
	blockhash,
} from "@solana/kit";
import { helloworldProgram } from "./util";
import { TransactionErrorFieldless } from "internal";

test("compute limit", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram(10n);
	const ix = {
		programAddress: programId,
		accounts: [
			{
				address: greetedPubkey,
				role: 1,
			},
		],
		data: new Uint8Array([0]),
	};
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(1_000_000_000n));
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	const tx = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([ix], tx),
	);
	const signedTx = await signTransactionMessageWithSigners(tx);
	const res = svm.sendTransaction(signedTx);
	if (res instanceof FailedTransactionMetadata) {
		assert.strictEqual(res.err(), TransactionErrorFieldless.AccountNotFound);
	} else {
		throw new Error("Expected transaction failure");
	}
});