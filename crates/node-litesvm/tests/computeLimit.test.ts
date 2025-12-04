import { generateKeyPairSigner, lamports } from "@solana/kit";
import { TransactionErrorFieldless } from "internal";
import { FailedTransactionMetadata, LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getGreetInstruction,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
	setComputeUnitLimit,
	setHelloWorldAccount,
	setHelloWorldProgram,
} from "./util";

test("compute limit", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress, greetedAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
		generateAddress(),
	]);

	// And a LiteSVM client with a CU limit set to 10.
	const svm = new LiteSVM()
		.tap(setComputeUnitLimit(10n))
		.tap((svm) => svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL)))
		.tap(setHelloWorldProgram(programAddress))
		.tap(setHelloWorldAccount(greetedAddress, programAddress));

	// When we send a greet instruction which uses more than 10 compute units.
	const transaction = await getSignedTransaction(svm, payer, [
		getGreetInstruction(greetedAddress, programAddress),
	]);
	const result = svm.sendTransaction(transaction);

	// Then we expect it to fail.
	if (result instanceof FailedTransactionMetadata) {
		assert.strictEqual(result.err(), TransactionErrorFieldless.AccountNotFound);
	} else {
		throw new Error("Expected transaction failure");
	}
});
