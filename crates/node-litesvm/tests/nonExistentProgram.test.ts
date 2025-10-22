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
	type Address,
} from "@solana/kit";

// Equivalent to LAMPORTS_PER_SOL from @solana/web3.js
const LAMPORTS_PER_SOL = 1_000_000_000n;

test("non-existent program", async () => {
	const svm = new LiteSVM();
	const nonExistentProgram = await generateKeyPairSigner();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	// Verify the program doesn't exist
	const programAccount = svm.getAccount(nonExistentProgram.address);
	assert.strictEqual(programAccount, null);
	
	// Verify payer has funds
	const payerBalance = svm.getBalance(payer.address);
	assert.strictEqual(payerBalance, LAMPORTS_PER_SOL);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	
	// Create an instruction calling a non-existent program
	const instruction = {
		programAddress: nonExistentProgram.address,
		accounts: [] as Array<{ address: Address; role: number }>,
		data: new Uint8Array([0]), // Some arbitrary data
	};
	
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	
	// This should fail because the program doesn't exist
	const result = svm.sendTransaction(signedTransaction);
	
	// Verify the transaction failed due to non-existent program
	assert.ok(result instanceof FailedTransactionMetadata, "Transaction should fail when calling non-existent program");
	
	// The error should indicate the program doesn't exist or is invalid
	const error = result.err();
	assert.ok(error !== null, "Should have an error when calling non-existent program");
});
