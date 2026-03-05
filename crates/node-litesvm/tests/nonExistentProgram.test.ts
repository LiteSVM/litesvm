import { generateKeyPairSigner, lamports } from "@solana/kit";
import { TransactionErrorFieldless } from "internal";
import { FailedTransactionMetadata, LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
} from "./util";

test("non-existent program", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);

	// And a LiteSVM client with no loaded programs.
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));

	// When we try to send a transaction to a non-existent program.
	const transaction = await getSignedTransaction(svm, payer, [
		{ data: new Uint8Array([0]), programAddress },
	]);
	const result = svm.sendTransaction(transaction);

	// Then we expect it to fail.
	if (result instanceof FailedTransactionMetadata) {
		const err = result.err();
		assert.strictEqual(
			err,
			TransactionErrorFieldless.InvalidProgramForExecution,
		);
	} else {
		throw new Error("Expected transaction failure");
	}
});
