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

const fs = require('fs');


test("clock", () => {
	// grab the "0::<path>" line
	const line = fs.readFileSync('/proc/self/cgroup', 'utf8')
		.split('\n')
		.find((l: string) => l.startsWith('0::'));

	if (line) {
		const cgPath = line.slice(3);        // strip the "0::"
		console.log('cgroup path:', cgPath);

		const read = (p: string) => fs.readFileSync(p, 'utf8').trim();
		console.log('memory.max =', read(`/sys/fs/cgroup${cgPath}/memory.max`));

		try {                                // memory.high is optional
			console.log('memory.high =', read(`/sys/fs/cgroup${cgPath}/memory.high`));
		} catch (_) { }
	}
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
