import { generateKeyPairSigner, lamports } from "@solana/kit";
import {
	FailedTransactionMetadata,
	LiteSVM,
	TransactionMetadata,
} from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
} from "./util";

test("clock", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);

	// And a LiteSVM client with a hello world program loaded from `litesvm_clock_example.so`.
	const svm = new LiteSVM()
		.tap((svm) => svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL)))
		.addProgramFromFile(
			programAddress,
			"program_bytes/litesvm_clock_example.so",
		);

	// And given two unique transactions.
	const [firstTransaction, secondTransaction] = await Promise.all([
		getSignedTransaction(svm, payer, [
			{ programAddress, data: new Uint8Array([0]) },
		]),
		getSignedTransaction(svm, payer, [
			{ programAddress, data: new Uint8Array([1]) },
		]),
	]);

	// When we set the time to January 1st 2000 and send the first transaction.
	const initialClock = svm.getClock();
	initialClock.unixTimestamp = 1735689600n;
	svm.setClock(initialClock);
	const firstResult = svm.sendTransaction(firstTransaction);

	// Then it fails because the contract wants it to be January 1970.
	if (firstResult instanceof FailedTransactionMetadata) {
		assert.ok(firstResult.err().toString().includes("ProgramFailedToComplete"));
	} else {
		throw new Error("Expected transaction failure here");
	}

	// When we set the time to January 1st 1970 and send the second transaction.
	const newClock = svm.getClock();
	newClock.unixTimestamp = 50n;
	svm.setClock(newClock);
	const secondResult = svm.sendTransaction(secondTransaction);

	// Then it succeeds.
	assert.ok(secondResult instanceof TransactionMetadata);
});
