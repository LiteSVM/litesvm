import {
	appendTransactionMessageInstruction,
	createTransactionMessage,
	generateKeyPairSigner,
	lamports,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";
import {
	FailedTransactionMetadata,
	LiteSVM,
	TransactionMetadata,
} from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateAddress, LAMPORTS_PER_SOL } from "./util";

test("clock", async () => {
	const programAddress = await generateAddress();
	const svm = new LiteSVM();
	svm.addProgramFromFile(
		programAddress,
		"program_bytes/litesvm_clock_example.so",
	);
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));

	const firstTransaction = await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => appendTransactionMessageInstruction({ programAddress }, tx),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash: svm.latestBlockhash(), lastValidBlockHeight: 0n },
				tx,
			),
		(tx) => signTransactionMessageWithSigners(tx),
	);

	// set the time to January 1st 2000
	const initialClock = svm.getClock();
	initialClock.unixTimestamp = 1735689600n;
	svm.setClock(initialClock);
	// this will fail because the contract wants it to be January 1970
	const failed = svm.sendTransaction(firstTransaction);
	if (failed instanceof FailedTransactionMetadata) {
		assert.ok(failed.err().toString().includes("ProgramFailedToComplete"));
	} else {
		throw new Error("Expected transaction failure here");
	}

	// so let's turn back time
	const newClock = svm.getClock();
	newClock.unixTimestamp = 50n;
	svm.setClock(newClock);

	const secondTransaction = await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) =>
			appendTransactionMessageInstruction(
				{
					programAddress,
					data: new Uint8Array([1]), // Unused, just here to dedup the transaction.
				},
				tx,
			),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash: svm.latestBlockhash(), lastValidBlockHeight: 0n },
				tx,
			),
		(tx) => signTransactionMessageWithSigners(tx),
	);

	// now the transaction goes through
	const success = svm.sendTransaction(secondTransaction);
	assert.ok(success instanceof TransactionMetadata);
});
