import { test } from "node:test";
import assert from "node:assert/strict";
import { address, generateKeyPairSigner } from "@solana/kit";
import { createLiteSVM, LiteSVMKit, addressFromBytes, addressToBytes } from "litesvm";

test("kit factory creates working instance", () => {
	const svm = createLiteSVM();
	assert.ok(svm);
	assert.equal(typeof svm.getAccount, "function");
	assert.equal(typeof svm.airdrop, "function");
	assert.equal(typeof svm.latestBlockhash, "function");
});

test("kit address conversion works", () => {
	const testAddress = address("11111111111111111111111111111111");
	const addressBytes = addressToBytes(testAddress);
	const addr = addressFromBytes(addressBytes);
	
	assert.equal(testAddress, addr);
});

test("kit airdrop functionality", () => {
	const svm = createLiteSVM();
	const recipient = address("11111111111111111111111111111112");
	const lamports = BigInt(1000000000);
	
	const result = svm.airdrop(recipient, lamports);
	assert.ok(result);
	assert.ok("signature" in result); // Should be successful transaction
	
	const balance = svm.getBalance(recipient);
	assert.equal(balance, lamports);
});

test("kit account management", () => {
	const svm = createLiteSVM();
	const testAddress = address("11111111111111111111111111111112");
	
	// Initially no account
	const initialAccount = svm.getAccount(testAddress);
	assert.equal(initialAccount, null);
	
	// Set an account
	const accountData = new Uint8Array([1, 2, 3, 4]);
	svm.setAccount(testAddress, {
		address: testAddress,
		executable: false,
		lamports: 1000000n,
		data: accountData,
		owner: address("11111111111111111111111111111111"), // System program
		rentEpoch: 0n,
	});
	
	// Verify account was set
	const account = svm.getAccount(testAddress);
	assert.ok(account);
	assert.equal(account.lamports, 1000000n);
	assert.equal(account.executable, false);
	assert.deepEqual(account.data, accountData);
});

test("kit extended class functionality", () => {
	const svm = new LiteSVMKit();
	const testAddress = address("11111111111111111111111111111112");
	
	// Test Kit-specific methods  
	const balance = svm.getBalance(testAddress);
	assert.ok(balance !== undefined);

	const account = svm.getAccount(testAddress);
	// Account doesn't exist initially
	assert.equal(account, null);
	
	// Set an account and verify
	svm.setAccount(testAddress, {
		address: testAddress,
		executable: false,
		lamports: BigInt(1000000000),
		data: new Uint8Array([1, 2, 3]),
		owner: address("11111111111111111111111111111111"),
		rentEpoch: 0n,
	});
	
	const newAccount = svm.getAccount(testAddress);
	assert.ok(newAccount);
	assert.equal(newAccount.lamports, BigInt(1000000000));
});

test("kit configuration methods return kit interface", () => {
	const svm = createLiteSVM();
	
	// Test that configuration methods return the kit interface
	const configured = svm
		.withSigverify(false)
		.withBlockhashCheck(false)
		.withSysvars()
		.withBuiltins();
	
	assert.ok(configured);
	assert.equal(typeof configured.getAccount, "function");
	assert.equal(typeof configured.sendTransaction, "function");
});

test("kit blockhash functionality", () => {
	const svm = createLiteSVM();
	
	const initialBlockhash = svm.latestBlockhash();
	assert.ok(initialBlockhash);
	assert.equal(typeof initialBlockhash, "string");
	
	svm.expireBlockhash();
	const newBlockhash = svm.latestBlockhash();
	assert.ok(newBlockhash);
	assert.notEqual(initialBlockhash, newBlockhash);
});

test("kit slot management", () => {
	const svm = createLiteSVM();
	
	const initialClock = svm.getClock();
	const initialSlot = initialClock.slot;
	
	const targetSlot = initialSlot + 100n;
	svm.warpToSlot(targetSlot);
	
	const updatedClock = svm.getClock();
	assert.equal(updatedClock.slot, targetSlot);
});