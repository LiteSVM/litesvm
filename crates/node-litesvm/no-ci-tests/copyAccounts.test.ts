import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import { PublicKey, Connection } from "@solana/web3.js";

test("copy accounts from devnet", async () => {
	const owner = PublicKey.unique();
	const usdcMint = new PublicKey(
		"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
	);
	const connection = new Connection("https://api.devnet.solana.com");
	const accountInfo = await connection.getAccountInfo(usdcMint);
	// the rent epoch goes above 2**53 which breaks web3.js, so just set it to 0;
	accountInfo.rentEpoch = 0;
	const svm = new LiteSVM();
	svm.setAccount(usdcMint, accountInfo);
	const rawAccount = svm.getAccount(usdcMint);
	assert.notStrictEqual(rawAccount, null);
});
