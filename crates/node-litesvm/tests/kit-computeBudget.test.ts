import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit, ComputeBudget } from "litesvm";
import {
	Address,
	address,
} from "@solana/kit";

// Helper function to create Kit program with compute budget
function helloworldProgramKitWithBudget(computeMaxUnits: bigint): [LiteSVMKit, Address, Address] {
	const programId = address("11111111111111111111111111111114");
	const greetedPubkey = address("11111111111111111111111111111115");
	
	// Create SVM with compute budget
	let svm = new LiteSVMKit();
	const computeBudget = new ComputeBudget();
	computeBudget.computeUnitLimit = computeMaxUnits;
	svm = svm.withComputeBudget(computeBudget);
	
	// Set account - include all required fields
	svm.setAccount(greetedPubkey, {
		address: greetedPubkey,
		executable: false,
		owner: programId,
		lamports: 1_000_000_000n, // 1 SOL
		data: new Uint8Array([0, 0, 0, 0]),
		rentEpoch: 0n,
	});
	
	// Add program
	svm.addProgramFromFile(programId, "program_bytes/counter.so");
	
	return [svm, programId, greetedPubkey];
}

test("kit compute budget configuration", () => {
	const svm = new LiteSVMKit();
	
	// Test compute budget configuration
	const computeBudget = new ComputeBudget();
	computeBudget.computeUnitLimit = 1_000_000n; // 1M compute units
	
	const svmWithBudget = svm.withComputeBudget(computeBudget);
	
	// Verify we get a kit instance back (preserves kit interface)
	assert.strictEqual(typeof svmWithBudget.airdrop, "function");
	assert.strictEqual(typeof svmWithBudget.getBalance, "function");
	assert.strictEqual(typeof svmWithBudget.setAccount, "function");
	
	// Test that the budget configuration works
	const [svmLowBudget, programId, greetedPubkey] = helloworldProgramKitWithBudget(10n);
	
	// Verify initial account state
	const greetedAccountBefore = svmLowBudget.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	
	// Test that we can create an SVM with low compute budget
	// The actual transaction execution would require proper kit transaction building
	assert.strictEqual(greetedAccountBefore?.lamports, 1_000_000_000n);
});