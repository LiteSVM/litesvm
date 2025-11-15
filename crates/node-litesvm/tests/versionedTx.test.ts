import { test } from "node:test";
import assert from "node:assert/strict";
import {
	generateKeyPairSigner,
	pipe,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	blockhash,
} from "@solana/kit";
import { helloworldProgram } from "./util";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("versioned tx", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, BigInt(LAMPORTS_PER_SOL));
	const ix = {
		programAddress: programId,
		accounts: [
			{
				address: greetedPubkey,
				role: 1,
			},
		],
		data: new Uint8Array([0]),
	};
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([ix], tx)
	);
	const transaction = await signTransactionMessageWithSigners(transactionMessage);
	svm.sendTransaction(transaction);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0])
	);
});