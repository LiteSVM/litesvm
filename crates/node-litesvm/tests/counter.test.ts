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
	lamports
} from "@solana/kit";
import { helloworldProgram, getLamports } from "./util";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("hello world", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const lamports = getLamports(svm, greetedPubkey);
	assert.strictEqual(lamports, LAMPORTS_PER_SOL);

	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);

	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
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
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	svm.sendTransaction(signedTransaction);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});