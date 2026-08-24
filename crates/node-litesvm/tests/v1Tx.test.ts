import { getTransferSolInstruction } from "@solana-program/system";
import {
	appendTransactionMessageInstruction,
	createTransactionMessage,
	generateKeyPairSigner,
	Instruction,
	lamports,
	pipe,
	setTransactionMessageComputeUnitLimit,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLoadedAccountsDataSizeLimit,
	signTransactionMessageWithSigners,
	TransactionSigner,
} from "@solana/kit";
import { LiteSVM, SimulatedTransactionInfo, TransactionMetadata } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateAddress, LAMPORTS_PER_SOL } from "./util";

const MAX_COMPUTE_UNIT_LIMIT = 1_400_000;
const MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES = 64 * 1024 * 1024;

// Unset resource limits in a version 1 transaction config are treated as
// zero, so the transaction sets generous values explicitly.
async function getSignedV1Transaction(
	svm: LiteSVM,
	payer: TransactionSigner,
	instruction: Instruction,
) {
	return await pipe(
		createTransactionMessage({ version: 1 }),
		(tx) => setTransactionMessageComputeUnitLimit(MAX_COMPUTE_UNIT_LIMIT, tx),
		(tx) =>
			setTransactionMessageLoadedAccountsDataSizeLimit(
				MAX_LOADED_ACCOUNTS_DATA_SIZE_BYTES,
				tx,
			),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => appendTransactionMessageInstruction(instruction, tx),
		(tx) => svm.setTransactionMessageLifetimeUsingLatestBlockhash(tx),
		(tx) => signTransactionMessageWithSigners(tx),
	);
}

test("send a version 1 transaction", async () => {
	// Given the following addresses and signers.
	const [payer, receiver] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);

	// And a LiteSVM client such that the payer has some balance.
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));

	// When we send a transfer instruction using a version 1 transaction.
	const transferredAmount = lamports(1_000_000n);
	const transaction = await getSignedV1Transaction(
		svm,
		payer,
		getTransferSolInstruction({
			source: payer,
			destination: receiver,
			amount: transferredAmount,
		}),
	);
	const result = svm.sendTransaction(transaction);

	// Then the transaction succeeds and the receiver has received the
	// transferred amount.
	assert.ok(result instanceof TransactionMetadata);
	const balanceAfter = svm.getBalance(receiver);
	assert.strictEqual(balanceAfter, transferredAmount);
});

test("simulate a version 1 transaction", async () => {
	// Given the following addresses and signers.
	const [payer, receiver] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
	]);

	// And a LiteSVM client such that the payer has some balance.
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));

	// When we simulate a transfer instruction using a version 1 transaction.
	const transferredAmount = lamports(1_000_000n);
	const transaction = await getSignedV1Transaction(
		svm,
		payer,
		getTransferSolInstruction({
			source: payer,
			destination: receiver,
			amount: transferredAmount,
		}),
	);
	const result = svm.simulateTransaction(transaction);

	// Then the simulation succeeds and no lamports have actually moved.
	assert.ok(result instanceof SimulatedTransactionInfo);
	const balanceAfter = svm.getBalance(receiver);
	assert.strictEqual(balanceAfter, null);
});
