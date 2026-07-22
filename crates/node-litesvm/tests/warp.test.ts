import { LiteSVM } from "litesvm/kit";
import assert from "node:assert/strict";
import { test } from "node:test";

test("warp", () => {
	const svm = new LiteSVM();
	const newSlot = 1000n;
	svm.warpToSlot(newSlot);
	const slot1 = svm.getClock().slot;
	assert.strictEqual(slot1, newSlot);
});
