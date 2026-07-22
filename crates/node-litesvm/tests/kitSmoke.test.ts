import { getTransferSolInstruction } from "@solana-program/system";
import { generateKeyPairSigner, lamports } from "@solana/kit";
import { FailedTransactionMetadata, LiteSVM } from "litesvm/kit";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
} from "./util";

test("kit subpath can airdrop, send, read state, and simulate", async () => {
	const svm = new LiteSVM();
	const [payer, receiver] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);

	const airdropResult = svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));
	assert(!(airdropResult instanceof FailedTransactionMetadata));

	const transferLamports = lamports(1_000_000n);
	const transaction = await getSignedTransaction(svm, payer, [
		getTransferSolInstruction({
			source: payer,
			destination: receiver,
			amount: transferLamports,
		}),
	]);

	const simulation = svm.simulateTransaction(transaction);
	assert(!(simulation instanceof FailedTransactionMetadata));

	const result = svm.sendTransaction(transaction);
	assert(!(result instanceof FailedTransactionMetadata));
	assert.strictEqual(svm.getBalance(receiver), transferLamports);

	const payerAccount = svm.getAccount(payer.address);
	assert.strictEqual(payerAccount.exists, true);
});
