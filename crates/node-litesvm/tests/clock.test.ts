console.log("In clock module");
import { test } from "node:test";
import assert from "node:assert/strict";
console.log("Doing litesvm imports");
import {
	FailedTransactionMetadata,
	LiteSVM,
	TransactionMetadata,
} from "litesvm";
console.log("Doing solana kit imports");
import {
	createTransactionMessage,
	appendTransactionMessageInstructions,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
	pipe,
	lamports,
	generateKeyPairSigner,
	blockhash,
} from "@solana/kit";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("clock", async () => {
	console.log("Running clock test");
	const programSigner = await generateKeyPairSigner();
	const programId = programSigner.address;
	console.log("Calling new LiteSVM()");
	const svm = new LiteSVM();
	console.log("Calling addProgramFromFile");
	svm.addProgramFromFile(programId, "program_bytes/litesvm_clock_example.so");
	console.log("Calling new Keypair");
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, BigInt(LAMPORTS_PER_SOL));
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const ixs = [
		{
			programAddress: programId,
			accounts: [] as const,
			data: new Uint8Array([]),
		},
	];
	const tx = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash: latestBlockhash, lastValidBlockHeight: 0n },
				tx,
			),
		(tx) => appendTransactionMessageInstructions(ixs, tx),
	);
	const signedTransaction = await signTransactionMessageWithSigners(tx);
	// set the time to January 1st 2000
	const initialClock = svm.getClock();
	initialClock.unixTimestamp = 1735689600n;
	svm.setClock(initialClock);
	// this will fail because the contract wants it to be January 1970
	const failed = svm.sendTransaction(signedTransaction);
	if (failed instanceof FailedTransactionMetadata) {
		assert.ok(failed.err().toString().includes("ProgramFailedToComplete"));
	} else {
		throw new Error("Expected transaction failure here");
	}
	// so let's turn back time
	const newClock = svm.getClock();
	newClock.unixTimestamp = 50n;
	svm.setClock(newClock);
	const ixs2 = [
		{
			programAddress: programId,
			accounts: [] as const,
			data: new Uint8Array(Buffer.from("foobar")), // unused, just here to dedup the tx
		},
	];
	const tx2 = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash: latestBlockhash, lastValidBlockHeight: 0n },
				tx,
			),
		(tx) => appendTransactionMessageInstructions(ixs2, tx),
	);
	const signedTransaction2 = await signTransactionMessageWithSigners(tx2);
	const success = svm.sendTransaction(signedTransaction2);
	assert.ok(success instanceof TransactionMetadata);
	console.log("Finished clock test");
});
