import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import type { Address } from "@solana/kit";
import type { KitAccountInfo } from "litesvm";

const LAMPORTS_PER_SOL = 1_000_000_000n;
const DEFAULT_OWNER = "11111111111111111111111111111111" as Address;

test("set account", () => {
	const svm = new LiteSVMKit();
	const address = "5xot9PVkphiX2adznghwrAuxGs2zeWisNSxMW6hU6Hkj" as Address;
	const toSet: KitAccountInfo = {
		address,
		executable: false,
		owner: DEFAULT_OWNER,
		lamports: LAMPORTS_PER_SOL,
		data: new Uint8Array([0, 1]),
		rentEpoch: 0n,
	};
	svm.setAccount(address, toSet);
	const fetched = svm.getAccount(address);
	assert.notStrictEqual(fetched, null);
	assert.deepStrictEqual(fetched?.data, new Uint8Array([0, 1]));
});
