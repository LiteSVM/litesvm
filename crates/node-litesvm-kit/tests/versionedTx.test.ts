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
	compileTransaction,
} from "@solana/kit";
import { getTransferSolInstruction } from "@solana-program/system";
import { helloworldProgram } from "./util";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("versioned tx", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const transferAmount = 1000n;
	
	// Create a simple transfer instruction for version 0 transaction
	const transferInstruction = getTransferSolInstruction({
		source: payer,
		destination: greetedPubkey,
		amount: transferAmount,
	});
	
	// Create version 0 transaction message
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([transferInstruction], tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	const compiledTransaction = compileTransaction(transactionMessage);
	
	// Send the versioned transaction
	const res = svm.sendTransaction(signedTransaction);
	
	// Verify the transfer worked
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.strictEqual(greetedAccountAfter?.lamports, LAMPORTS_PER_SOL + transferAmount);
});
