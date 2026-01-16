import { NATIVE_MINT, NATIVE_MINT_2022 } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";

test("create native mints", () => {
	let svm = LiteSVM.default();

	assert.strictEqual(svm.getAccount(NATIVE_MINT), null, "SPL Token native mint should not exist");
	assert.strictEqual(svm.getAccount(NATIVE_MINT_2022), null, "Token-2022 native mint should not exist");

	svm = svm.withSysvars()
		.withDefaultPrograms()
		.withNativeMints();

	const validateData = (data: Uint8Array, mint: PublicKey) => {
		assert.ok(data.filter(x => x !== 0).length > 0, `${mint.toBase58()} data should not be empty`);
	};

	const nativeMint = svm.getAccount(NATIVE_MINT);
	assert.ok(nativeMint, "SPL Token native mint should exist");
	validateData(nativeMint.data, NATIVE_MINT);
	assert.ok(nativeMint.lamports > 0, "SPL Token native mint should have lamports");

	const nativeMint2022 = svm.getAccount(NATIVE_MINT_2022);
	assert.ok(nativeMint2022, "Token-2022 native mint should exist");
	validateData(nativeMint2022.data, NATIVE_MINT_2022);
	assert.ok(nativeMint2022.lamports > 0, "Token-2022 native mint should have lamports");
});
