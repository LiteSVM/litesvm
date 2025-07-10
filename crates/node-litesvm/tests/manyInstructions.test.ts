import { test } from "node:test";
import assert from "node:assert/strict";
import {
	LAMPORTS_PER_SOL,
	Transaction,
	TransactionInstruction,
	Keypair,
} from "@solana/web3.js";
import { helloworldProgram } from "./util";

test("many instructions", () => {
	const [svm, programId, greetedPubkey] = helloworldProgram();
	const ix = new TransactionInstruction({
		keys: [{ pubkey: greetedPubkey, isSigner: false, isWritable: true }],
		programId,
		data: Buffer.from([0]),
	});
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	const numIxs = 64;
	const ixs = Array(numIxs).fill(ix);
	const tx = new Transaction();
	tx.recentBlockhash = blockhash;
	tx.add(...ixs);
	tx.sign(payer);
	svm.sendTransaction(tx);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([64, 0, 0, 0]),
	);
});
