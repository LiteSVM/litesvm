import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import {
	Address,
	address,
} from "@solana/kit";

test("kit basic functionality", () => {
	const svm = new LiteSVMKit();
	
	// Generate addresses
	const payer = address("11111111111111111111111111111119");
	const receiver = address("1111111111111111111111111111111A");
	
	// Airdrop to payer
	svm.airdrop(payer, 1_000_000_000n); // 1 SOL
	
	// Check initial balances
	const payerBalance = svm.getBalance(payer);
	const receiverBalance = svm.getBalance(receiver);
	
	assert.strictEqual(payerBalance, 1_000_000_000n);
	assert.strictEqual(receiverBalance, 0n);
	
	// Test blockhash functionality
	const blockhash = svm.latestBlockhash();
	assert.strictEqual(typeof blockhash, "string");
	assert.strictEqual(blockhash.length, 44); // Base58 blockhash length
	
	// Test slot functionality
	const clock = svm.getClock();
	const slot = clock.slot;
	assert.strictEqual(typeof slot, "bigint");
	
	// Test warp functionality
	svm.warpToSlot(slot + 100n);
	const newClock = svm.getClock();
	assert.strictEqual(newClock.slot, slot + 100n);
});