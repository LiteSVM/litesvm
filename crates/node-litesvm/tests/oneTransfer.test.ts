import { getTransferSolInstruction } from "@solana-program/system";
import { generateKeyPairSigner, lamports } from "@solana/kit";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
} from "./util";

test("one transfer", async () => {
	// Given the following addresses and signers.
	const [payer, receiver] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);

	// And a LiteSVM client such that the payer has some balance.
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));

	// When we send a transfer instruction.
	const transferredAmount = lamports(1_000_000n);
	const transaction = await getSignedTransaction(svm, payer, [
		getTransferSolInstruction({
			source: payer,
			destination: receiver,
			amount: transferredAmount,
		}),
	]);
	svm.sendTransaction(transaction);

	// Then the receiver has received the transferred amount.
	const balanceAfter = svm.getBalance(receiver);
	assert.strictEqual(balanceAfter, transferredAmount);
});
