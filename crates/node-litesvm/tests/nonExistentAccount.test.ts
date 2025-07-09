import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import { PublicKey } from "@solana/web3.js";

test("non-existent account", () => {
	const svm = new LiteSVM();
	const acc = svm.getAccount(PublicKey.unique());
	assert.strictEqual(acc, null);
});
