import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";

test("warp", () => {
	const svm = new LiteSVM();
	const slot0 = svm.getClock().slot;
	assert.strictEqual(slot0, 0n);
	const newSlot = 1000n;
	svm.warpToSlot(newSlot);
	const slot1 = svm.getClock().slot;
	assert.strictEqual(slot1, newSlot);
});
