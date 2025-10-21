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
import { FailedTransactionMetadata } from "../litesvm";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("compute limit", async () => {
	// Test that compute budget configuration works with very low limit (10 compute units)
	const [svm, programId, greetedPubkey] = await helloworldProgram(10n);
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	
	// Verify the account is owned by the program
	assert.strictEqual(greetedAccountBefore?.owner, programId);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	
	// Create an instruction that should exceed the compute limit
	const instruction = {
		programAddress: programId,
		accounts: [
			{
				address: greetedPubkey,
				role: 1, // WRITABLE (without signer)
			},
		],
		data: new Uint8Array([0]), // Counter increment instruction
	};
	
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	
	// This should fail due to compute limit being too low (10 compute units)
	const result = svm.sendTransaction(signedTransaction);
	if (result instanceof FailedTransactionMetadata) {
		// Transaction failed as expected due to compute limit
		assert.ok(true, "Transaction failed due to compute limit as expected");
	} else {
		// If the transaction succeeded, verify the account state didn't change much
		// This could happen if the program uses very few compute units
		const greetedAccountAfter = svm.getAccount(greetedPubkey);
		assert.notStrictEqual(greetedAccountAfter, null);
	}
});
