import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, Rent, Clock } from "litesvm";

test("sysvar", () => {
	const svm = new LiteSVM();
	const rentBefore = svm.getRent();
	assert.strictEqual(rentBefore.burnPercent, 50);
	assert.strictEqual(rentBefore.minimumBalance(123n), 1746960n);
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
