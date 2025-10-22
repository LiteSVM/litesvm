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
} from "@solana/kit";
import { helloworldProgram } from "./util";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("many instructions", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	
	// Verify account setup
	assert.strictEqual(greetedAccountBefore?.owner, programId);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const numInstructions = 64;
	
	// Create many identical instructions to test batch processing
	const instructions = Array(numInstructions).fill(null).map(() => ({
		programAddress: programId,
		accounts: [
			{
				address: greetedPubkey,
				role: 1, // WRITABLE (without signer)
			},
		],
		data: new Uint8Array([0]), // Counter increment instruction
	}));
	
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions(instructions, tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	
	svm.sendTransaction(signedTransaction);
	
	// After 64 increment instructions, the counter should be 64
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([64, 0, 0, 0]), // 64 in little-endian format
	);
});
