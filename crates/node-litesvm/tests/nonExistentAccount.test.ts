import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import { generateAddress } from "./util";

test("non-existent account", async () => {
	// Given a LiteSVM client and an address that does not point to any account.
	const address = await generateAddress();
	const svm = new LiteSVM();

	// When we try to get a non-existent account.
	const account = svm.getAccount(address);

	// Then we expect a MaybeAccount with exists set to false.
	assert.strictEqual(account.exists, false);
	assert.strictEqual(account.address, address);
});
