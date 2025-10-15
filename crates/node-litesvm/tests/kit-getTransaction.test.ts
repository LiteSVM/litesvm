import { test } from "node:test";
import assert from "node:assert/strict";
import {
	AccountRole,
	appendTransactionMessageInstructions,
	createTransactionMessage,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
    generateKeyPairSigner,
    type Blockhash, Signature
} from "@solana/kit";
import { helloworldProgram, getLamports } from "./kit-util";
import { TransactionMetadata } from "internal";
import bs58 from "bs58";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("hello world", async () => {
	const [svm, programId, greetedPubkey] = await helloworldProgram();
	const lamports = getLamports(svm, greetedPubkey);
	assert.strictEqual(lamports, LAMPORTS_PER_SOL);
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	const blockhash = svm.latestBlockhash() as Blockhash;
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore?.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	
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
						accounts: [{ address: greetedPubkey, role: AccountRole.WRITABLE }],
						data: new Uint8Array([0]),
					},
				],
				msg,
			),
	);
	
	const result = await svm.sendTransaction(tx);
	assert.ok(result instanceof TransactionMetadata);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountAfter, null);
	assert.deepStrictEqual(
		greetedAccountAfter?.data,
		new Uint8Array([1, 0, 0, 0]),
	);
	// For Kit, we know result is TransactionMetadata since the assert passed
	const signature = bs58.encode((result as TransactionMetadata).signature()) as Signature;
	const fetched = svm.getTransaction(signature);
	
	// For Kit tests, getTransaction returns a Kit-compatible plain object, not a class instance
	assert.notStrictEqual(fetched, null);
	assert.strictEqual(typeof fetched, 'object');
	assert.strictEqual(fetched.signature, signature);
	assert.ok(fetched.logMessages.length > 0);
	assert.strictEqual(typeof fetched.computeUnitsConsumed, 'bigint');
});
