import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { Address, address } from "@solana/kit";

test("kit infinite usdc mint", () => {
	const svm = new LiteSVMKit();
	
	// Create addresses
	const owner = address("11111111111111111111111111111114"); // Valid base58 address
	const usdcMint = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"); // Real USDC mint
	const ata = address("11111111111111111111111111111115"); // Mock ATA address
	
	const usdcToOwn = 1_000_000_000_000n; // 1 trillion USDC (6 decimals)
	
	// Create mock token account data (simplified version)
	// Real SPL token account would have more complex structure
	const tokenAccData = new Uint8Array(165); // SPL token account size
	
	// Write mock data structure (simplified):
	// mint (32 bytes), owner (32 bytes), amount (8 bytes), etc.
	const mintBytes = new TextEncoder().encode(usdcMint).slice(0, 32);
	const ownerBytes = new TextEncoder().encode(owner).slice(0, 32);
	
	tokenAccData.set(mintBytes, 0);
	tokenAccData.set(ownerBytes, 32);
	
	// Write amount as little-endian 64-bit
	const amountView = new DataView(tokenAccData.buffer, 64, 8);
	amountView.setBigUint64(0, usdcToOwn, true); // little-endian
	
	// Set state to initialized (1)
	tokenAccData[108] = 1;
	
	// Set account in SVM - include all required fields
	svm.setAccount(ata, {
		address: ata,
		lamports: 1_000_000_000n, // 1 SOL for rent
		data: tokenAccData,
		owner: address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), // Token program ID
		executable: false,
		rentEpoch: 0n,
	});
	
	// Verify account was set correctly
	const rawAccount = svm.getAccount(ata);
	assert.notStrictEqual(rawAccount, null);
	assert.strictEqual(rawAccount?.lamports, 1_000_000_000n);
	assert.strictEqual(rawAccount?.data.length, 165);
	
	// Verify amount in account data
	const rawAccountData = rawAccount?.data;
	if (rawAccountData) {
		const amountFromAccount = new DataView(rawAccountData.buffer, 64, 8).getBigUint64(0, true);
		assert.strictEqual(amountFromAccount, usdcToOwn);
	}
});