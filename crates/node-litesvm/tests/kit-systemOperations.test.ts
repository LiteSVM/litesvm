import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import {
	Address,
	address,
} from "@solana/kit";

test("kit set account", () => {
	const svm = new LiteSVMKit();
	
	// Create test addresses
	const testAccount = address("11111111111111111111111111111116");
	const owner = address("11111111111111111111111111111117");
	
	// Set account with Kit-compatible types - include all required fields
	svm.setAccount(testAccount, {
		address: testAccount,
		lamports: 500_000_000n, // 0.5 SOL
		data: new Uint8Array([1, 2, 3, 4, 5]),
		owner: owner,
		executable: false,
		rentEpoch: 0n,
	});
	
	// Verify account was set correctly
	const account = svm.getAccount(testAccount);
	assert.notStrictEqual(account, null);
	assert.strictEqual(account?.lamports, 500_000_000n);
	assert.deepStrictEqual(account?.data, new Uint8Array([1, 2, 3, 4, 5]));
	assert.strictEqual(account?.owner, owner);
	assert.strictEqual(account?.executable, false);
	
	// Test balance retrieval
	const balance = svm.getBalance(testAccount);
	assert.strictEqual(balance, 500_000_000n);
});

test("kit non-existent account", () => {
	const svm = new LiteSVMKit();
	
	const nonExistentAccount = address("11111111111111111111111111111118");
	
	// Non-existent account should return null
	const account = svm.getAccount(nonExistentAccount);
	assert.strictEqual(account, null);
	
	// Balance should be 0 for non-existent account
	const balance = svm.getBalance(nonExistentAccount);
	assert.strictEqual(balance, 0n);
});

test("kit airdrop functionality", () => {
	const svm = new LiteSVMKit();
	
	const recipient = address("11111111111111111111111111111119");
	const airdropAmount = 2_000_000_000n; // 2 SOL
	
	// Initial balance should be 0
	assert.strictEqual(svm.getBalance(recipient), 0n);
	
	// Airdrop lamports
	svm.airdrop(recipient, airdropAmount);
	
	// Verify balance after airdrop
	const balance = svm.getBalance(recipient);
	assert.strictEqual(balance, airdropAmount);
	
	// Multiple airdrops should accumulate
	svm.airdrop(recipient, 1_000_000_000n); // +1 SOL
	const newBalance = svm.getBalance(recipient);
	assert.strictEqual(newBalance, 3_000_000_000n); // 3 SOL total
});