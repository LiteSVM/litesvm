import { test } from "node:test";
import assert from "node:assert/strict";
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
import { helloworldProgram, getLamports } from "./util";
import { TransactionMetadata } from "internal";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("hello world", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();

	const lamportsBefore = getLamports(svm, greetedPubkey);
	assert.strictEqual(lamportsBefore, LAMPORTS_PER_SOL);

	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);

	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0])
	);
	assert.strictEqual(greetedAccountBefore?.owner, programId);
	assert.strictEqual(greetedAccountBefore?.lamports, LAMPORTS_PER_SOL);
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const instruction = {
		programAddress: programId,
		accounts: [
			{
				address: greetedPubkey,
				role: 1,
			},
		],
		data: new Uint8Array([0]),
	};
	const txMsg = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash: latestBlockhash, lastValidBlockHeight: 0n },
				tx
			),
		(tx) => appendTransactionMessageInstructions([instruction], tx)
	);
	const signedTx = await signTransactionMessageWithSigners(txMsg);
	const txResult = svm.sendTransaction(signedTx);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0])
	);
	const signature =
		txResult instanceof TransactionMetadata
			? txResult.signature()
			: txResult.meta().signature();
	const fetched = svm.getTransaction(signature);
	assert.ok(fetched instanceof TransactionMetadata);
});
