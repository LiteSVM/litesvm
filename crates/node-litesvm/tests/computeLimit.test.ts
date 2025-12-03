import { test } from "node:test";
import assert from "node:assert/strict";
import { FailedTransactionMetadata } from "litesvm";
import { helloworldProgram, LAMPORTS_PER_SOL } from "./util";
import { TransactionErrorFieldless } from "internal";
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

test("compute limit", async () => {
	const [svm, programAddress, greetedAddress] = await helloworldProgram(10n);
	const ix: Instruction = {
		accounts: [{ address: greetedAddress, role: AccountRole.WRITABLE }],
		programAddress: programAddress,
		data: new Uint8Array([0]),
	};
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));
	const greetedAccountBefore = svm.getAccount(greetedAddress);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore.data,
		new Uint8Array([0, 0, 0, 0]),
	);

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

	const res = svm.sendTransaction(transaction);
	if (res instanceof FailedTransactionMetadata) {
		assert.strictEqual(res.err(), TransactionErrorFieldless.AccountNotFound);
	} else {
		throw new Error("Expected transaction failure");
	}
});
