import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, AccountInfoBytes } from "litesvm";
import { lamports, address } from "@solana/kit";

test("set account", () => {
	const svm = new LiteSVM();
	const pubkey = address("5xot9PVkphiX2adznghwrAuxGs2zeWisNSxMW6hU6Hkj");
	const data = new Uint8Array([0, 1]);
	const toSet: AccountInfoBytes = {
		executable: false,
		owner: pubkey,
		lamports: lamports(1_000_000_000n),
		data,
		space: BigInt(data.length),
	};
	svm.setAccount(pubkey, toSet);
	const fetched = svm.getAccount(pubkey);
	assert.notStrictEqual(fetched, null);
	assert.deepStrictEqual(fetched!.data, data);
});
