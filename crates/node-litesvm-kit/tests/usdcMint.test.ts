import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { address, generateKeyPairSigner, type Address } from "@solana/kit";

test("create account with custom data", async () => {
	const owner = await generateKeyPairSigner();
	const usdcMint = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
	const testAccount = await generateKeyPairSigner();
	
	const customData = new Uint8Array([1, 2, 3, 4, 5]);
	const svm = new LiteSVMKit();
	
	svm.setAccount(testAccount.address, {
		lamports: 1_000_000_000n,
		data: customData,
		owner: usdcMint,
		executable: false,
		rentEpoch: 0n,
	});
	
	const rawAccount = svm.getAccount(testAccount.address);
	assert.notStrictEqual(rawAccount, null);
	assert.deepStrictEqual(rawAccount?.data, customData);
	assert.strictEqual(rawAccount?.owner, usdcMint);
	assert.strictEqual(rawAccount?.lamports, 1_000_000_000n);
});
