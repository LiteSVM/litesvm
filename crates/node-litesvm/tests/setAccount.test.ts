import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, AccountInfoBytes } from "litesvm";
import { address, type Address } from "@solana/kit";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("set account", () => {
	const svm = new LiteSVM();
	const testAddress = address("5xot9PVkphiX2adznghwrAuxGs2zeWisNSxMW6hU6Hkj");
	const defaultOwner = address("11111111111111111111111111111111"); // System program
	const toSet: AccountInfoBytes = {
		executable: false,
		owner: defaultOwner,
		lamports: LAMPORTS_PER_SOL,
		data: new Uint8Array([0, 1]),
		rentEpoch: 0n,
	};
	svm.setAccount(testAddress, toSet);
	const fetched = svm.getAccount(testAddress);
	assert.notStrictEqual(fetched, null);
	assert.deepStrictEqual(fetched?.data, new Uint8Array([0, 1]));
});
