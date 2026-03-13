import { EncodedAccount, lamports } from "@solana/kit";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateAddress, LAMPORTS_PER_SOL } from "./util";

test("set account", async () => {
	// Given the following addresses.
	const [accountAddress, programAddress] = await Promise.all([
		generateAddress(),
		generateAddress(),
	]);

	// When we set an account in the LiteSVM.
	const svm = new LiteSVM();
	const account: EncodedAccount = {
		address: accountAddress,
		executable: false,
		lamports: lamports(LAMPORTS_PER_SOL),
		programAddress,
		data: new Uint8Array([0, 1]),
		space: 2n,
	};
	svm.setAccount(account);

	// Then we can fetch the account and it matches what we set.
	const fetched = svm.getAccount(accountAddress);
	assert.deepStrictEqual(fetched, { exists: true, ...account });
});
