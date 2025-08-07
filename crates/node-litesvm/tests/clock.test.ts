console.log("In clock module");
import { test } from "node:test";
import assert from "node:assert/strict";
console.log("Doing litesvm imports");
import {
	FailedTransactionMetadata,
	LiteSVM,
	TransactionMetadata,
} from "litesvm";
console.log("Doing web3.js imports");
import {
	Keypair,
	LAMPORTS_PER_SOL,
	PublicKey,
	Transaction,
	TransactionInstruction,
} from "@solana/web3.js";

test("clock", () => {
	console.log("Running clock test");
	const programId = PublicKey.unique();
	console.log("Calling new LiteSVM()");
	const svm = new LiteSVM();
	console.log("Calling addProgramFromFile");
	svm.addProgramFromFile(programId, "program_bytes/litesvm_clock_example.so");
	console.log("Calling new Keypair");
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const ixs = [
		new TransactionInstruction({ keys: [], programId, data: Buffer.from("") }),
	];
	const tx = new Transaction();
	tx.recentBlockhash = blockhash;
	tx.add(...ixs);
	tx.sign(payer);
	// set the time to January 1st 2000
	const initialClock = svm.getClock();
	initialClock.unixTimestamp = 1735689600n;
	svm.setClock(initialClock);
	// this will fail because the contract wants it to be January 1970
	const failed = svm.sendTransaction(tx);
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
		new TransactionInstruction({
			keys: [],
			programId,
			data: Buffer.from("foobar"), // unused, just here to dedup the tx
		}),
	];
	const tx2 = new Transaction();
	tx2.recentBlockhash = blockhash;
	tx2.add(...ixs2);
	tx2.sign(payer);
	// now the transaction goes through
	const success = svm.sendTransaction(tx2);
	assert.ok(success instanceof TransactionMetadata);
	console.log("Finished clock test");
});
