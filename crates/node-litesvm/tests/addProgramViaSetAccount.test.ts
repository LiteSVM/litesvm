import { test } from "node:test";
import assert from "node:assert/strict";
import {
	LAMPORTS_PER_SOL,
	Transaction,
	TransactionInstruction,
	Keypair,
} from "@solana/web3.js";
import { helloworldProgramViaSetAccount } from "./util";

test("add program via setAccount", () => {
	const [svm, programId, greetedPubkey] = helloworldProgramViaSetAccount();
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	const ix = new TransactionInstruction({
		keys: [{ pubkey: greetedPubkey, isSigner: false, isWritable: true }],
		programId,
		data: Buffer.from([0]),
	});
	const tx = new Transaction();
	tx.recentBlockhash = blockhash;
	tx.add(ix);
	tx.sign(payer);
	svm.sendTransaction(tx);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});
