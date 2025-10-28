import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import {
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	lamports,
	blockhash,
} from "@solana/kit";
import { getTransferSolInstruction } from "@solana-program/system";

test("one transfer", async () => {
	const svm = new LiteSVM();
	const sender = await generateKeyPairSigner();
	const recipient = await generateKeyPairSigner();
	const LAMPORTS_PER_SOL = 1_000_000_000n;
	const transferAmount = lamports(LAMPORTS_PER_SOL / 100n);
	svm.airdrop(sender.address, lamports(LAMPORTS_PER_SOL));
	const senderBalance = svm.getBalance(sender.address);
	assert.strictEqual(senderBalance, lamports(LAMPORTS_PER_SOL));
	const recipientBalanceBefore = svm.getBalance(recipient.address);
	assert.strictEqual(recipientBalanceBefore, null);
	const transferInstruction = getTransferSolInstruction({
		source: sender,
		destination: recipient.address,
		amount: transferAmount,
	});
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(sender, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({
			blockhash: latestBlockhash,
			lastValidBlockHeight: 0n
		}, tx),
		(tx) => appendTransactionMessageInstructions([transferInstruction], tx),
	);
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	svm.sendTransaction(signedTransaction);
	const balanceAfter = svm.getBalance(recipient.address);
	assert.strictEqual(balanceAfter, transferAmount);
});