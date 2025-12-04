import {
	AccountState,
	findAssociatedTokenPda,
	getTokenDecoder,
	getTokenEncoder,
	Token,
	TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import {
	address,
	assertAccountExists,
	decodeAccount,
	generateKeyPairSigner,
	lamports,
	none,
} from "@solana/kit";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateAddress, LAMPORTS_PER_SOL } from "./util";

test("infinite usdc mint", async () => {
	// Given the following addresses and signers.
	const [payer, owner] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);
	const usdcMint = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

	// And a LiteSVM client such that the payer has some balance.
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));

	// Add the following associated token account for the owner.
	const [ata] = await findAssociatedTokenPda({
		owner,
		mint: usdcMint,
		tokenProgram: TOKEN_PROGRAM_ADDRESS,
	});

	// And the following token account data.
	const tokenAccountData: Token = {
		mint: usdcMint,
		owner,
		amount: 1_000_000_000_000n,
		delegate: none(),
		state: AccountState.Initialized,
		isNative: none(),
		delegatedAmount: 0n,
		closeAuthority: none(),
	};
	const encodedTokenAccountData = getTokenEncoder().encode(tokenAccountData);

	// When we set that associated token account on the LiteSVM.
	svm.setAccount({
		address: ata,
		lamports: lamports(LAMPORTS_PER_SOL),
		programAddress: TOKEN_PROGRAM_ADDRESS,
		executable: false,
		data: encodedTokenAccountData,
		space: BigInt(encodedTokenAccountData.length),
	});

	// Then we can fetch the account and it matches what we set.
	const fetchedAccount = decodeAccount(svm.getAccount(ata), getTokenDecoder());
	assertAccountExists(fetchedAccount);
	assert.deepStrictEqual(fetchedAccount.data, tokenAccountData);
});
