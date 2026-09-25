import {
	FailedTransactionMetadata,
	LiteSVM,
	TransactionMetadata,
	type AccountInfoBytes,
} from "litesvm-web3js";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	Address,
	Keypair,
	LAMPORTS_PER_SOL,
	SystemProgram,
	Transaction,
	TransactionMessage,
	VersionedTransaction,
} from "@solana/web3.js";

test("web3.js legacy transactions can simulate, send, and read state", async () => {
	const svm = new LiteSVM();
	const [payer, recipientKeypair, dataKeypair] = await Promise.all([
		Keypair.generate(),
		Keypair.generate(),
		Keypair.generate(),
	]);
	const recipient = recipientKeypair.publicKey;

	const airdropResult = svm.airdrop(
		payer.publicKey,
		BigInt(LAMPORTS_PER_SOL),
	);
	assert(!(airdropResult instanceof FailedTransactionMetadata));

	const transferLamports = 1_000_000n;
	const transaction = new Transaction({
		feePayer: payer.publicKey,
		blockhash: svm.latestBlockhash(),
		lastValidBlockHeight: 0n,
	}).add(
		SystemProgram.transfer({
			fromPubkey: payer.publicKey,
			toPubkey: recipient,
			lamports: transferLamports,
		}),
	);
	await transaction.sign(payer);

	const simulation = await svm.simulateTransaction(transaction);
	assert(!(simulation instanceof FailedTransactionMetadata));

	const result = await svm.sendTransaction(transaction);
	assert(result instanceof TransactionMetadata);
	assert.strictEqual(svm.getBalance(recipient), transferLamports);
	assert(transaction.signature !== null);
	assert(svm.getTransaction(transaction.signature) instanceof TransactionMetadata);

	const recipientAccount = svm.getAccount(recipient);
	assert(recipientAccount !== null);
	assert(recipientAccount.owner instanceof Address);
	assert.equal(recipientAccount.space, 0n);

	const account: AccountInfoBytes = {
		executable: false,
		owner: Address.default,
		lamports: BigInt(LAMPORTS_PER_SOL),
		data: new Uint8Array([1, 2, 3]),
		rentEpoch: 0n,
		space: 3n,
	};
	svm.setAccount(dataKeypair.publicKey, account);
	assert.deepStrictEqual(svm.getAccount(dataKeypair.publicKey)?.data, account.data);
	const programAccounts = svm.getProgramAccounts(Address.default);
	assert(
		programAccounts.some(
			([address, programAccount]) =>
				address.equals(dataKeypair.publicKey) &&
				programAccount.space === account.space,
		),
	);
});

test("web3.js v0 transactions use the versioned native path", async () => {
	const svm = new LiteSVM();
	const [payer, recipient] = await Promise.all([
		Keypair.generate(),
		Keypair.generate(),
	]);
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));

	const transferLamports = 2_000_000n;
	const message = new TransactionMessage({
		payerKey: payer.publicKey,
		recentBlockhash: svm.latestBlockhash(),
		instructions: [
			SystemProgram.transfer({
				fromPubkey: payer.publicKey,
				toPubkey: recipient.publicKey,
				lamports: transferLamports,
			}),
		],
	}).compileToV0Message();
	const transaction = new VersionedTransaction(message);
	await transaction.sign([payer]);

	const result = await svm.sendTransaction(transaction);
	assert(result instanceof TransactionMetadata);
	assert.equal(svm.getBalance(recipient.publicKey), transferLamports);
	assert(svm.getTransaction(transaction.signatures[0]) instanceof TransactionMetadata);
});

test("disabled signature verification accepts an invalid legacy signature", async () => {
	const svm = new LiteSVM().withSigverify(false);
	const [payer, recipient] = await Promise.all([
		Keypair.generate(),
		Keypair.generate(),
	]);
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));

	const transaction = new Transaction({
		feePayer: payer.publicKey,
		blockhash: svm.latestBlockhash(),
		lastValidBlockHeight: 0n,
	}).add(
		SystemProgram.transfer({
			fromPubkey: payer.publicKey,
			toPubkey: recipient.publicKey,
			lamports: 1_000_000n,
		}),
	);
	await transaction.sign(payer);
	transaction.signatures[0].signature = new Uint8Array(64).fill(42);

	const result = await svm.sendTransaction(transaction);
	assert(result instanceof TransactionMetadata);
	assert.equal(svm.getBalance(recipient.publicKey), 1_000_000n);
});
