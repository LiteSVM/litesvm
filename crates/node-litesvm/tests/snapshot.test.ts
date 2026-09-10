import { getTransferSolInstruction } from "@solana-program/system";
import {
	assertAccountExists,
	decodeAccount,
	generateKeyPairSigner,
	getSignatureFromTransaction,
	lamports,
} from "@solana/kit";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
	generateAddress,
	getCounterDecoder,
	getGreetInstruction,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
	setHelloWorldAccount,
	setHelloWorldProgram,
} from "./util";

// Deploys the counter program, seeds a payer and a counter account, and
// records one transaction so that every part of the state is exercised.
async function setUpEnvironment() {
	const [payer, programAddress, greetedAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
		generateAddress(),
	]);
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));
	setHelloWorldProgram(svm, programAddress);
	setHelloWorldAccount(svm, greetedAddress, programAddress);
	const transaction = await getSignedTransaction(svm, payer, [
		getGreetInstruction(greetedAddress, programAddress),
	]);
	svm.sendTransaction(transaction);
	const signature = getSignatureFromTransaction(transaction);
	return { svm, payer, programAddress, greetedAddress, signature };
}

function getCount(svm: LiteSVM, address: Parameters<LiteSVM["getAccount"]>[0]) {
	const account = decodeAccount(svm.getAccount(address), getCounterDecoder());
	assertAccountExists(account);
	return account.data.count;
}

test("snapshot round trip through bytes", async () => {
	// Given an environment with a program, some accounts and one transaction.
	const { svm, payer, programAddress, greetedAddress, signature } =
		await setUpEnvironment();

	// When we snapshot it and restore the snapshot into a new instance.
	const restored = LiteSVM.fromBytes(svm.toBytes());

	// Then the restored instance has the same accounts, blockhash and history.
	assert.strictEqual(getCount(restored, greetedAddress), 1);
	assert.strictEqual(
		restored.getBalance(payer.address),
		svm.getBalance(payer.address),
	);
	assert.strictEqual(restored.latestBlockhash(), svm.latestBlockhash());
	assert.notStrictEqual(restored.getTransaction(signature), null);

	// And the program deployed before the snapshot still runs. The blockhash
	// is expired first so the new transaction does not collide with the one
	// already in the restored history.
	restored.expireBlockhash();
	const transaction = await getSignedTransaction(restored, payer, [
		getGreetInstruction(greetedAddress, programAddress),
	]);
	restored.sendTransaction(transaction);
	assert.strictEqual(getCount(restored, greetedAddress), 2);
});

test("snapshot round trip through a file", async () => {
	const { svm, greetedAddress } = await setUpEnvironment();
	const dir = mkdtempSync(join(tmpdir(), "litesvm-snapshot-"));
	try {
		const path = join(dir, "state.bin");
		svm.saveToFile(path);
		const restored = LiteSVM.loadFromFile(path);
		assert.strictEqual(getCount(restored, greetedAddress), 1);
		assert.strictEqual(restored.latestBlockhash(), svm.latestBlockhash());
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
});

test("restored instances are independent", async () => {
	// Given a snapshot restored twice, as a test suite would before each test.
	const { svm, payer } = await setUpEnvironment();
	const snapshot = svm.toBytes();
	const [first, second] = [
		LiteSVM.fromBytes(snapshot),
		LiteSVM.fromBytes(snapshot),
	];
	const balanceBefore = svm.getBalance(payer.address);

	// When we spend from one of the restored instances.
	const receiver = await generateAddress();
	const transaction = await getSignedTransaction(first, payer, [
		getTransferSolInstruction({
			source: payer,
			destination: receiver,
			amount: lamports(1_000_000n),
		}),
	]);
	first.sendTransaction(transaction);

	// Then neither the original nor the other restored instance is affected.
	assert.strictEqual(first.getBalance(receiver), lamports(1_000_000n));
	assert.strictEqual(second.getBalance(receiver), null);
	assert.strictEqual(svm.getBalance(receiver), null);
	assert.strictEqual(second.getBalance(payer.address), balanceBefore);
});

test("fromBytes rejects data that is not a snapshot", () => {
	assert.throws(
		() => LiteSVM.fromBytes(new Uint8Array([255, 1, 2, 3])),
		/Failed to load snapshot/,
	);
});
