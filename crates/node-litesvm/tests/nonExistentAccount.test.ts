import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import { generateKeyPairSigner } from "@solana/kit";

test("non-existent account", async () => {
	const svm = new LiteSVM();
	const keypair = await generateKeyPairSigner();
	const acc = svm.getAccount(keypair.address);
	assert.strictEqual(acc, null);
});