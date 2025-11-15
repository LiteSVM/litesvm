import { test } from "node:test";
import assert from "node:assert/strict";
import {
	lamports,
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	blockhash,
} from "@solana/kit";
import { helloworldProgram } from "./util";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("test sigverify", async () => {
	let [svm, programId, greetedPubkey] = await helloworldProgram();
	svm = svm.withSigverify(false);
	const realSigner = await generateKeyPairSigner();
	svm.airdrop(realSigner.address, LAMPORTS_PER_SOL);

	const latestBlockhash = blockhash(svm.latestBlockhash());
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	const instruction = {
		programAddress: programId,
		accounts: [
			{
				address: greetedPubkey,
				role: 1
			},
		],
		data: new Uint8Array([0]),
	};
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(realSigner, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({
			blockhash: latestBlockhash,
			lastValidBlockHeight: 0n
		}, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	const originalSignatures = Object.values(signedTransaction.signatures);
	const fakeSignature = originalSignatures[0];
	const corruptedSignatureBytes = new Uint8Array(fakeSignature.length);
	const corruptedSignature = Object.assign(corruptedSignatureBytes, fakeSignature);
	const corruptedTransaction = {
		...signedTransaction,
		signatures: {
			[realSigner.address]: corruptedSignature,
		},
	};
	svm.sendTransaction(corruptedTransaction);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});