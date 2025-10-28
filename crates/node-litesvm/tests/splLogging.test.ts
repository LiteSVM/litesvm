import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, TransactionMetadata } from "litesvm";
import {
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	lamports,
	blockhash,
} from "@solana/kit";

test("spl logging", async () => {
	const programSigner = await generateKeyPairSigner();
	const programId = programSigner.address;
	const svm = new LiteSVM();
	svm.addProgramFromFile(programId, "program_bytes/spl_example_logging.so");
	const payer = await generateKeyPairSigner();
	const LAMPORTS_PER_SOL = lamports(1_000_000_000n);
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const randomAccount = await generateKeyPairSigner();
	const instruction = {
		programAddress: programId,
		accounts: [
			{
				address: randomAccount.address,
				role: 0,
			},
		],
		data: new Uint8Array(0),
	};
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ 
			blockhash: latestBlockhash, 
			lastValidBlockHeight: 0n 
		}, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	// let's sim it first
	const simRes = svm.simulateTransaction(signedTransaction);
	const sendRes = svm.sendTransaction(signedTransaction);
	if (sendRes instanceof TransactionMetadata) {
		assert.deepStrictEqual(simRes.meta().logs(), sendRes.logs());
		assert.strictEqual(sendRes.logs()[1], "Program log: static string");
	} else {
		throw new Error("Unexpected tx failure");
	}
});