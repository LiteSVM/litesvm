import { test } from "node:test";
import assert from "node:assert/strict";
import {
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	blockhash,
	lamports,
} from "@solana/kit";
import { helloworldProgram } from "./util";

test("many instructions", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const ix = {
		programAddress: programId,
		accounts: [{ address: greetedPubkey, role: 1 }],
		data: new Uint8Array([0]),
	};
	const payer = await generateKeyPairSigner();
	const LAMPORTS_PER_SOL = lamports(1_000_000_000n);
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	const numIxs = 64;
	const ixs = Array(numIxs).fill(ix);
	const tx = pipe(
		createTransactionMessage({ version: 0 }),
		(m) => setTransactionMessageFeePayerSigner(payer, m),
		(m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, m),
		(m) => appendTransactionMessageInstructions(ixs, m),
	);
	const signed = await signTransactionMessageWithSigners(tx);
	svm.sendTransaction(signed);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([64, 0, 0, 0]),
	);
});