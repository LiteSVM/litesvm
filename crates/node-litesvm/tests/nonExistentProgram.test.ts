import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, FailedTransactionMetadata } from "litesvm";
import {
	PublicKey,
	LAMPORTS_PER_SOL,
	Transaction,
	TransactionInstruction,
	Keypair,
} from "@solana/web3.js";
import { TransactionErrorFieldless } from "internal";

test("non-existent program", () => {
	const svm = new LiteSVM();
	const ix = new TransactionInstruction({
		data: Buffer.alloc(1),
		keys: [],
		programId: PublicKey.unique(),
	});
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const tx = new Transaction().add(ix);
	tx.recentBlockhash = svm.latestBlockhash();
	tx.sign(payer);
	const res = svm.sendTransaction(tx);
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
