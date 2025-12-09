import {
	assertAccountExists,
	decodeAccount,
	generateKeyPairSigner,
	lamports,
} from "@solana/kit";
import { LiteSVM } from "index";
import assert from "node:assert/strict";
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

const NUMBER_OF_INSTRUCTIONS = 64;

test("many instructions", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress, greetedAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
		generateAddress(),
	]);

	// And a LiteSVM client with a hello world program and greeted account set up.
	const svm = new LiteSVM();
	svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL));
	setHelloWorldProgram(svm, programAddress);
	setHelloWorldAccount(svm, greetedAddress, programAddress);

	// And given the greeted account has 0 greets.
	const greetedAccountBefore = decodeAccount(
		svm.getAccount(greetedAddress),
		getCounterDecoder(),
	);
	assertAccountExists(greetedAccountBefore);
	assert.deepStrictEqual(greetedAccountBefore.data.count, 0);

	// When we send a transaction with many greet instructions.
	const instructions = Array(NUMBER_OF_INSTRUCTIONS).fill(
		getGreetInstruction(greetedAddress, programAddress),
	);
	const transaction = await getSignedTransaction(svm, payer, instructions);
	svm.sendTransaction(transaction);

	// Then we expect the greeted account to have been greeted many times.
	const greetedAccountAfter = decodeAccount(
		svm.getAccount(greetedAddress),
		getCounterDecoder(),
	);
	assertAccountExists(greetedAccountAfter);
	assert.deepStrictEqual(
		greetedAccountAfter.data.count,
		NUMBER_OF_INSTRUCTIONS,
	);
});
