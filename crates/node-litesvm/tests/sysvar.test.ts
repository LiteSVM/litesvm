import { Clock, LiteSVM, Rent, SlotHash, StakeHistoryEntry } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";

test("sysvar", () => {
	const svm = new LiteSVM();
	const rentBefore = svm.getRent();
	assert.strictEqual(rentBefore.burnPercent, 50);
	assert.strictEqual(rentBefore.minimumBalance(123n), 1275080n);
	const newRent = new Rent(
		rentBefore.lamportsPerByteYear,
		rentBefore.exemptionThreshold,
		0,
	);
	svm.setRent(newRent);
	const rentAfter = svm.getRent();
	assert.strictEqual(rentAfter.burnPercent, 0);
	const clockBefore = svm.getClock();
	assert.strictEqual(clockBefore.epoch, 0n);
	const newClock = new Clock(1000n, 1n, 100n, 3n, 4n);
	svm.setClock(newClock);
	const clockAfter = svm.getClock();
	assert.strictEqual(clockAfter.epoch, newClock.epoch);
});

test("setSlotHashes accepts object literals", () => {
	const svm = new LiteSVM();
	const slotHashes = [
		{
			slot: 1000n,
			hash: "G4caYtkdHZW5aPWokD6r1E3pnAxJmvXBsw3JtQFkMY8z",
		},
	];

	svm.setSlotHashes(slotHashes);

	const [slotHash] = svm.getSlotHashes();
	assert(slotHash instanceof SlotHash);
	assert.strictEqual(slotHash.slot, slotHashes[0].slot);
	assert.strictEqual(slotHash.hash, slotHashes[0].hash);
});

test("StakeHistoryEntry setters write their own fields", () => {
	const entry = new StakeHistoryEntry(1n, 2n, 3n);
	entry.activating = 20n;
	assert.strictEqual(entry.effective, 1n);
	assert.strictEqual(entry.activating, 20n);
	assert.strictEqual(entry.deactivating, 3n);
	entry.deactivating = 30n;
	assert.strictEqual(entry.effective, 1n);
	assert.strictEqual(entry.activating, 20n);
	assert.strictEqual(entry.deactivating, 30n);
	entry.effective = 10n;
	assert.strictEqual(entry.effective, 10n);
	assert.strictEqual(entry.activating, 20n);
	assert.strictEqual(entry.deactivating, 30n);
});
