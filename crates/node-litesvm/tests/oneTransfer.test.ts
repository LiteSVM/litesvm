import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
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
import { getTransferSolInstruction } from "@solana-program/system";

test("one transfer", async () => {
	const svm = new LiteSVM();
	const payer = await generateKeyPairSigner();
	const LAMPORTS_PER_SOL = lamports(1_000_000_000n);
	
	// Test airdrop functionality
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const payerBalance = svm.getBalance(payer.address);
	assert.strictEqual(payerBalance, LAMPORTS_PER_SOL);
	
	// Test balance checking for non-existent account
	const receiver = address("11111111111111111111111111111112");
	const receiverBalance = svm.getBalance(receiver);
	assert.strictEqual(receiverBalance, null);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const transferLamports = 1_000_000n;
	
	const transferInstruction = getTransferSolInstruction({
		source: payer,
		destination: receiver,
		amount: transferLamports,
	});
	
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([transferInstruction], tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	
	svm.sendTransaction(signedTransaction);
	const balanceAfter = svm.getBalance(receiver);
	assert.strictEqual(balanceAfter, transferLamports);
});
