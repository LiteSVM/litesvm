import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { type Address } from "@solana/kit";
import { generateKeyPairSigner } from "@solana/signers";
import {
	TOKEN_PROGRAM_ADDRESS,
	findAssociatedTokenPda,
	getTokenEncoder,
	getTokenDecoder,
} from "@solana-program/token";

const ONE_SOL = 1_000_000_000n;

test("infinite usdc mint", async () => {
	const ownerSigner = await generateKeyPairSigner();
	const owner: Address = ownerSigner.address;
	
	const usdcMint: Address = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" as Address;
	
	const [ataAddress] = await findAssociatedTokenPda({
		mint: usdcMint,
		owner,
		tokenProgram: TOKEN_PROGRAM_ADDRESS,
	});
	
	const usdcToOwn = 1_000_000_000_000n;
	const encodeTokenAccount = getTokenEncoder();
	// Ensure proper conversion of ReadonlyUint8Array to Uint8Array
	const tokenAccData = new Uint8Array(encodeTokenAccount.encode({
		mint: usdcMint,
		owner,
		amount: usdcToOwn,
		delegate: "11111111111111111111111111111111" as Address,
		delegatedAmount: 0n, // Default value
		state: 1, // Initialized
		isNative: 0n,
		closeAuthority: "11111111111111111111111111111111" as Address,
	}));
	
	const svm = new LiteSVMKit();
	
	// Write the ATA directly into the VM, owned by the SPL Token program
	svm.setAccount(ataAddress, {
		lamports: ONE_SOL, 
		data: tokenAccData, 
		owner: TOKEN_PROGRAM_ADDRESS,
		executable: false,
		rentEpoch: 0n,
	});
	
	const rawAccount = svm.getAccount(ataAddress);
	assert.notStrictEqual(rawAccount, null);
	const accountData = new Uint8Array(rawAccount.data);
	
	const decodeTokenAccount = getTokenDecoder();
	const decoded = decodeTokenAccount.decode(accountData);
	
	assert.strictEqual(decoded.amount, usdcToOwn);
});