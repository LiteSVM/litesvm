import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM } from "litesvm";
import {
	generateKeyPairSigner,
	address,
	lamports,
} from "@solana/kit";
import {
	TOKEN_PROGRAM_ADDRESS,
	findAssociatedTokenPda,
	getTokenEncoder,
	getTokenDecoder,
	AccountState,
} from "@solana-program/token";

test("infinite usdc mint", async () => {
	const owner = await generateKeyPairSigner();
	const usdcMint = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
	const [ata] = await findAssociatedTokenPda({
		mint: usdcMint,
		owner: owner.address,
		tokenProgram: TOKEN_PROGRAM_ADDRESS,
	});

	const usdcToOwn = 1_000_000_000_000n;

	const tokenAccountData = {
		mint: usdcMint,
		owner: owner.address,
		amount: usdcToOwn,
		delegate: null as any,
		state: AccountState.Initialized,
		isNative: null as any,
		delegatedAmount: 0n,
		closeAuthority: null as any,
	};
	
	const encoder = getTokenEncoder();
	const tokenAccData = new Uint8Array(encoder.encode(tokenAccountData));
	const svm = new LiteSVM();

	svm.setAccount(ata, {
		lamports: lamports(1_000_000_000n),
		data: tokenAccData,
		owner: TOKEN_PROGRAM_ADDRESS,
		executable: false,
		space: BigInt(tokenAccData.length),
	});

	const rawAccount = svm.getAccount(ata);
	assert.notStrictEqual(rawAccount, null, "Token account should exist");
	assert.strictEqual(rawAccount!.owner, TOKEN_PROGRAM_ADDRESS, "Account should be owned by token program");
	assert.strictEqual(rawAccount!.lamports, lamports(1_000_000_000n), "Account should have correct lamports");

	const decoder = getTokenDecoder();
	const decodedTokenAccount = decoder.decode(rawAccount!.data);
	assert.strictEqual(decodedTokenAccount.amount, usdcToOwn, "Token account should contain the correct USDC amount");
	assert.strictEqual(decodedTokenAccount.mint, usdcMint, "Token account should be associated with USDC mint");
	assert.strictEqual(decodedTokenAccount.owner, owner.address, "Token account should be owned by the correct address");
});