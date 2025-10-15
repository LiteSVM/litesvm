import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { generateKeyPairSigner } from "@solana/kit";

test("non-existent account", async () => {
	const svm = new LiteSVMKit();
	// Generate a random keypair to get a unique address that doesn't exist in the SVM
	const randomKeypair = await generateKeyPairSigner();
	const acc = svm.getAccount(randomKeypair.address);
	assert.strictEqual(acc, null);
});
