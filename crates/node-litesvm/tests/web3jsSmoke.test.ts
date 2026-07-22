import {
	FailedTransactionMetadata,
	LiteSVM,
	type AccountInfoBytes,
} from "litesvm/web3js";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	Address,
	Keypair,
	LAMPORTS_PER_SOL,
	SystemProgram,
	Transaction,
} from "@solana/web3.js";

test("web3js subpath can airdrop, send, read state, and simulate", async () => {
	const svm = new LiteSVM();
	const [payer, receiverKeypair] = await Promise.all([
		Keypair.generate(),
		Keypair.generate(),
	]);
	const receiver = receiverKeypair.publicKey;

	const airdropResult = svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	assert(!(airdropResult instanceof FailedTransactionMetadata));

	const transferLamports = 1_000_000n;
	const transaction = new Transaction({
		feePayer: payer.publicKey,
		blockhash: svm.latestBlockhash(),
		lastValidBlockHeight: 0n,
	}).add(
		SystemProgram.transfer({
			fromPubkey: payer.publicKey,
			toPubkey: receiver,
			lamports: transferLamports,
		}),
	);
	await transaction.sign(payer);

	const simulation = await svm.simulateTransaction(transaction);
	assert(!(simulation instanceof FailedTransactionMetadata));

	const result = await svm.sendTransaction(transaction);
	assert(!(result instanceof FailedTransactionMetadata));
	assert.strictEqual(svm.getBalance(receiver), transferLamports);

	const receiverAccount = svm.getAccount(receiver);
	assert(receiverAccount !== null);
	assert(receiverAccount.owner instanceof Address);

	const dataAddress = (await Keypair.generate()).publicKey;
	const account: AccountInfoBytes = {
		executable: false,
		owner: Address.default,
		lamports: BigInt(LAMPORTS_PER_SOL),
		data: new Uint8Array([1, 2, 3]),
	};
	svm.setAccount(dataAddress, account);
	assert.deepStrictEqual(svm.getAccount(dataAddress)?.data, account.data);
});
