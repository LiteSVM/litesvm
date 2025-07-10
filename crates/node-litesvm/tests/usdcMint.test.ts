import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import { PublicKey } from "@solana/web3.js";
import {
	getAssociatedTokenAddressSync,
	AccountLayout,
	ACCOUNT_SIZE,
	TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

test("infinite usdc mint", () => {
	const owner = PublicKey.unique();
	const usdcMint = new PublicKey(
		"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
	);
	const ata = getAssociatedTokenAddressSync(usdcMint, owner, true);
	const usdcToOwn = 1_000_000_000_000n;
	const tokenAccData = Buffer.alloc(ACCOUNT_SIZE);
	AccountLayout.encode(
		{
			mint: usdcMint,
			owner,
			amount: usdcToOwn,
			delegateOption: 0,
			delegate: PublicKey.default,
			delegatedAmount: 0n,
			state: 1,
			isNativeOption: 0,
			isNative: 0n,
			closeAuthorityOption: 0,
			closeAuthority: PublicKey.default,
		},
		tokenAccData,
	);
	const svm = new LiteSVM();
	svm.setAccount(ata, {
		lamports: 1_000_000_000,
		data: tokenAccData,
		owner: TOKEN_PROGRAM_ID,
		executable: false,
	});
	const rawAccount = svm.getAccount(ata);
	assert.notStrictEqual(rawAccount, null);
	const rawAccountData = rawAccount?.data;
	const decoded = AccountLayout.decode(rawAccountData);
	assert.strictEqual(decoded.amount, usdcToOwn);
});
