import { test } from "node:test";
import assert from "node:assert/strict";
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
import { helloworldProgramViaSetAccount, LAMPORTS_PER_SOL } from "./util";

test("add program via setAccount", async () => {
	const [svm, programId, greetedPubkey] =
		await helloworldProgramViaSetAccount();
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);

	const instruction: Instruction = {
		accounts: [{ address: greetedPubkey, role: AccountRole.WRITABLE }],
		programAddress: programId,
		data: Buffer.from([0]),
	};

	const transaction = await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => appendTransactionMessageInstruction(instruction, tx),
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash, lastValidBlockHeight: 0n },
				tx,
			),
		(tx) => signTransactionMessageWithSigners(tx),
	);

	svm.sendTransaction(transaction);

	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
});
