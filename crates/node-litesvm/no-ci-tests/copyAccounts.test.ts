import { address, assertAccountExists, createSolanaRpc, fetchEncodedAccount } from "@solana/kit";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";

test("copy accounts from devnet", async () => {
	const usdcMint = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
	const rpc = createSolanaRpc("https://api.devnet.solana.com");
	const account = await fetchEncodedAccount(rpc, usdcMint);
	assertAccountExists(account);

	const svm = new LiteSVM();
	svm.setAccount(account);
	const rawAccount = svm.getAccount(usdcMint);
	assert.notStrictEqual(rawAccount, null);
});
