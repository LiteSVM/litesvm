import { test } from "node:test";
import assert from "node:assert/strict";
import {
	LAMPORTS_PER_SOL,
	Transaction,
	TransactionInstruction,
	PublicKey,
} from "@solana/web3.js";
import { helloworldProgram } from "./util";

test("test sigverify", () => {
	let [svm, programId, greetedPubkey] = helloworldProgram();
	svm = svm.withSigverify(false);
	const payerPubkey = new PublicKey(12345);
	const fakeSigner = {
		publicKey: payerPubkey,
		secretKey: new Uint8Array(32),
	}; // note that the secretKey & publicKey do not match
	svm.airdrop(payerPubkey, BigInt(LAMPORTS_PER_SOL));
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
	tx.sign(fakeSigner);
	svm.sendTransaction(tx);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});
