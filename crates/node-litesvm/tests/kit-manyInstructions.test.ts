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

test("many instructions", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const blockhash = svm.latestBlockhash() as Blockhash;
	
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	
	// Create the instruction that will be repeated
	const instruction = {
		programAddress: programId,
		accounts: [{ address: greetedPubkey, role: AccountRole.WRITABLE }],
		data: new Uint8Array([0]),
	};
	
	const numIxs = 64;
	const instructions = Array(numIxs).fill(instruction);
	
	const tx = pipe(
		createTransactionMessage({ version: 0 }),
		(msg) => setTransactionMessageFeePayerSigner(payer, msg),
		(msg) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash, lastValidBlockHeight: 0n },
				msg,
			),
		(msg) => appendTransactionMessageInstructions(instructions, msg),
	);
	
	await svm.sendTransaction(tx);
	
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([64, 0, 0, 0]),
	);
});
