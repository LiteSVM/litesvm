import { test } from "node:test";
import assert from "node:assert/strict";
import {
	AccountRole,
	appendTransactionMessageInstructions,
	createTransactionMessage,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
    generateKeyPairSigner,
    type Blockhash
} from "@solana/kit";
import { helloworldProgram } from "./kit-util";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("versioned tx", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const blockhash = svm.latestBlockhash() as Blockhash;
	
	// Create a versioned transaction message (version 0) using Solana Kit
	const tx = pipe(
		createTransactionMessage({ version: 0 }), // This creates a versioned transaction
		(msg) => setTransactionMessageFeePayerSigner(payer, msg),
		(msg) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash, lastValidBlockHeight: 0n },
				msg,
			),
		(msg) =>
			appendTransactionMessageInstructions(
				[
					{
						programAddress: programId,
						accounts: [{ address: greetedPubkey, role: AccountRole.WRITABLE }],
						data: new Uint8Array([0]),
					},
				],
				msg,
			),
	);
	
	const res = await svm.sendTransaction(tx);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});
