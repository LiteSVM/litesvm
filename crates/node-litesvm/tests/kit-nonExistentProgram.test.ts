import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit, FailedTransactionMetadata } from "litesvm";
import {
	appendTransactionMessageInstructions,
	createTransactionMessage,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
    generateKeyPairSigner,
    type Blockhash
} from "@solana/kit";
import { TransactionErrorFieldless } from "internal";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("non-existent program", async () => {
	const svm = new LiteSVMKit();
	
	// Generate a random program ID that doesn't exist
	const nonExistentProgram = await generateKeyPairSigner();
	const programId = nonExistentProgram.address;
	
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const blockhash = svm.latestBlockhash() as Blockhash;
	
	const tx = pipe(
		createTransactionMessage({ version: 0 }),
		(msg) => setTransactionMessageFeePayerSigner(payer, msg),
		(msg) =>
			setTransactionMessageLifetimeUsingBlockhash(
				{ blockhash, lastValidBlockHeight: 0n },
				msg,
			),
		(msg) =>
			appendTransactionMessageInstructions(
				[
					{
						programAddress: programId,
						accounts: [],
						data: new Uint8Array(1), // Buffer.alloc(1) equivalent
					},
				],
				msg,
			),
	);
	
	const res = await svm.sendTransaction(tx);
	if (res instanceof FailedTransactionMetadata) {
		const err = res.err();
		assert.strictEqual(
			err,
			TransactionErrorFieldless.InvalidProgramForExecution,
		);
	} else {
		throw new Error("Expected transaction failure");
	}
});
