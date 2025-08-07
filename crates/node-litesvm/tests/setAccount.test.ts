import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, AccountInfoBytes } from "litesvm";
import { PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";

test("set account", () => {
	const svm = new LiteSVM();
	const address = new PublicKey("5xot9PVkphiX2adznghwrAuxGs2zeWisNSxMW6hU6Hkj");
	const toSet: AccountInfoBytes = {
		executable: false,
		owner: PublicKey.default,
		lamports: LAMPORTS_PER_SOL,
		data: new Uint8Array([0, 1]),
	};
	svm.setAccount(address, toSet);
	const fetched = svm.getAccount(address);
	assert.deepStrictEqual(fetched.data, new Uint8Array([0, 1]));
});
