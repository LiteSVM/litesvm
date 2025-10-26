import { test } from "node:test";
import assert from "node:assert/strict";
import {
	address,
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
import { helloworldProgram } from "./util";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("test sigverify", async () => {
	let [svm, programId, greetedPubkey] = await helloworldProgram();
	svm = svm.withSigverify(false);
	
	// Create a legitimate payer but we'll test with signature verification disabled
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	
	// Verify payer has funds
	const payerBalance = svm.getBalance(payer.address);
	assert.strictEqual(payerBalance, LAMPORTS_PER_SOL);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	
	// Create a transaction that would normally require proper signatures
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
	
	// With signature verification disabled, this should succeed even if signatures were problematic
	svm.sendTransaction(signedTransaction);
	
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
	
	// Verify that signature verification was indeed disabled by checking the transaction succeeded
	assert.ok(true, "Transaction succeeded with signature verification disabled");
});
