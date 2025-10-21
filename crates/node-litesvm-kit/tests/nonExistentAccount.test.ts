import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { generateKeyPairSigner } from "@solana/kit";

test("non-existent account", async () => {
	const svm = new LiteSVMKit();
	const keyPair = await generateKeyPairSigner();
	const acc = svm.getAccount(keyPair.address);
	assert.strictEqual(acc, null);
});
