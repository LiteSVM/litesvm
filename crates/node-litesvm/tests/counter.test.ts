import { test } from "node:test";
import assert from "node:assert/strict";
import { helloworldProgram, getLamports, LAMPORTS_PER_SOL } from "./util";
import {
	AccountRole,
	appendTransactionMessageInstruction,
	createTransactionMessage,
	generateKeyPairSigner,
	Instruction,
	lamports,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";

test("hello world", async () => {
	const [svm, programAddress, greetedAddress] = await helloworldProgram();
	assert.strictEqual(
		getLamports(svm, greetedAddress),
		lamports(LAMPORTS_PER_SOL),
	);
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const greetedAccountBefore = svm.getAccount(greetedAddress);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);

	const ix: Instruction = {
		accounts: [{ address: greetedAddress, role: AccountRole.WRITABLE }],
		programAddress,
		data: new Uint8Array([0]),
	};

	const transaction = await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => appendTransactionMessageInstruction(ix, tx),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash: svm.latestBlockhash(), lastValidBlockHeight: 0n },
				tx,
			),
		(tx) => signTransactionMessageWithSigners(tx),
	);

	svm.sendTransaction(transaction);
	const greetedAccountAfter = svm.getAccount(greetedAddress);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});
