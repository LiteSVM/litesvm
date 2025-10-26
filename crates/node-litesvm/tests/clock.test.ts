import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, Clock } from "litesvm";
import { generateKeyPairSigner, lamports } from "@solana/kit";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("clock", async () => {
	const programId = await generateKeyPairSigner();
	const svm = new LiteSVM();
	svm.addProgramFromFile(programId.address, "program_bytes/litesvm_clock_example.so");

	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);

	// Test clock manipulation
	const initialClock = svm.getClock();
	assert.strictEqual(initialClock.epoch, 0n);

	// Set the time to January 1st 2000
	const modifiedClock = new Clock(
		initialClock.slot,
		initialClock.epochStartTimestamp,
		initialClock.epoch,
		initialClock.leaderScheduleEpoch,
		1735689600n // January 1st 2000
	);
	svm.setClock(modifiedClock);

	let clockAfterUpdate = svm.getClock();
	assert.strictEqual(clockAfterUpdate.unixTimestamp, 1735689600n);

	// Turn back time 
	const earlierClock = new Clock(
		initialClock.slot,
		initialClock.epochStartTimestamp,
		initialClock.epoch,
		initialClock.leaderScheduleEpoch,
		50n // Early timestamp
	);
	svm.setClock(earlierClock);

	clockAfterUpdate = svm.getClock();
	assert.strictEqual(clockAfterUpdate.unixTimestamp, 50n);
});
