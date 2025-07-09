import { test } from "node:test";
import assert from "node:assert/strict";
import { FailedTransactionMetadata } from "litesvm";
import {
	LAMPORTS_PER_SOL,
	Transaction,
	TransactionInstruction,
	Keypair,
} from "@solana/web3.js";
import { helloworldProgram } from "./util";
import { TransactionErrorFieldless } from "internal";

test("compute limit", () => {
	const [svm, programId, greetedPubkey] = helloworldProgram(10n);
	const ix = new TransactionInstruction({
		keys: [{ pubkey: greetedPubkey, isSigner: false, isWritable: true }],
		programId,
		data: Buffer.from([0]),
	});
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const greetedAccountBefore = svm.getAccount(greetedPubkey);
	assert.notStrictEqual(greetedAccountBefore, null);
	assert.deepStrictEqual(
		greetedAccountBefore.data,
		new Uint8Array([0, 0, 0, 0]),
	);
	const tx = new Transaction();
	tx.recentBlockhash = blockhash;
	tx.add(ix);
	tx.sign(payer);
	const res = svm.sendTransaction(tx);
	if (res instanceof FailedTransactionMetadata) {
		assert.strictEqual(res.err(), TransactionErrorFieldless.AccountNotFound);
	} else {
		throw new Error("Expected transaction failure");
	}
});
