const assert = require("node:assert/strict");
const { test } = require("node:test");
const { Account, LiteSvm, TransactionMetadata } = require("..");

test("core bindings work without a Solana JavaScript SDK", () => {
	const svm = new LiteSvm();
	const address = new Uint8Array(32).fill(1);
	const owner = new Uint8Array(32);
	const data = new Uint8Array([1, 2, 3]);

	assert.equal(svm.getAccount(address), null);
	assert(svm.airdrop(address, 1_000_000n) instanceof TransactionMetadata);
	assert.equal(svm.getBalance(address), 1_000_000n);

	svm.setAccount(address, new Account(2_000_000n, data, owner, false, 0n));
	const account = svm.getAccount(address);
	assert.deepEqual(account.data(), data);
	assert.deepEqual(account.owner(), owner);
	assert.equal(account.lamports(), 2_000_000n);
});
