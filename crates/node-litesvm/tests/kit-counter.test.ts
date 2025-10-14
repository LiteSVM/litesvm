import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import {
	Address,
	address,
} from "@solana/kit";

// Helper function to create a Kit utility similar to the web3.js util
function helloworldProgramKit(): [LiteSVMKit, Address, Address] {
	const svm = new LiteSVMKit();
	
	// Generate unique addresses (Kit equivalent of PublicKey.unique())
	const programId = address("11111111111111111111111111111116"); // System program as example
	const greetedPubkey = address("11111111111111111111111111111117"); // Another valid address
	
	// Set account with Kit types - include all required fields
	svm.setAccount(greetedPubkey, {
		address: greetedPubkey,
		executable: false,
		owner: programId,
		lamports: 1_000_000_000n, // 1 SOL equivalent
		data: new Uint8Array([0, 0, 0, 0]),
		rentEpoch: 0n,
	});
	
	// Add program from file
	svm.addProgramFromFile(programId, "program_bytes/counter.so");
	
	return [svm, programId, greetedPubkey];
}

test("kit hello world counter setup", () => {
	const [svm, programId, greetedPubkey] = helloworldProgramKit();
	
	// Check initial account state
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	assert.strictEqual(greetedAccountBefore?.lamports, 1_000_000_000n);
	assert.strictEqual(greetedAccountBefore?.executable, false);
	assert.strictEqual(greetedAccountBefore?.owner, programId);
	
	// Test airdrop functionality
	const testReceiver = address("11111111111111111111111111111118");
	svm.airdrop(testReceiver, 500_000_000n);
	const balance = svm.getBalance(testReceiver);
	assert.strictEqual(balance, 500_000_000n);
});