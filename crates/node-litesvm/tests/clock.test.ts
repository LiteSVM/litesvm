import { test } from "node:test";
import assert from "node:assert/strict";
import {
	FailedTransactionMetadata,
	LiteSVM,
	TransactionMetadata,
} from "litesvm";
import {
	Keypair,
	LAMPORTS_PER_SOL,
	PublicKey,
	Transaction,
	TransactionInstruction,
} from "@solana/web3.js";

const v8  = require('v8');


test("clock", () => {

	setInterval(() => {
		const m = process.memoryUsage();
		console.log(
		  `rss=${(m.rss/1048576).toFixed(1)} MB  ` +
		  `heapUsed=${(m.heapUsed/1048576).toFixed(1)} MB  ` +
		  `external=${(m.external/1048576).toFixed(1)} MB`,
		  '   avail=', (v8.getHeapStatistics().total_available_size/1048576).toFixed(1), 'MB'
		);
	  }, 10);
	const programId = PublicKey.unique();
	const svm = new LiteSVM();
	svm.addProgramFromFile(programId, "program_bytes/litesvm_clock_example.so");
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
});
