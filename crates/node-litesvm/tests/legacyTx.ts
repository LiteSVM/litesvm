import {
	appendTransactionMessageInstruction,
	assertAccountExists,
	createTransactionMessage,
	decodeAccount,
	generateKeyPairSigner,
	lamports,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";
import { LiteSVM } from "index";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getCounterDecoder,
	getGreetInstruction,
	LAMPORTS_PER_SOL,
	setHelloWorldAccount,
	setHelloWorldProgram,
} from "./util";

test("legacy tx", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress, greetedAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
		generateAddress(),
	]);

	// And a LiteSVM client with a hello world program and greeted account set up.
	const svm = new LiteSVM()
		.tap((svm) => svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL)))
		.tap(setHelloWorldProgram(programAddress))
		.tap(setHelloWorldAccount(greetedAddress, programAddress));

	// And given the greeted account has 0 greets.
	const greetedAccountBefore = decodeAccount(svm.getAccount(greetedAddress), getCounterDecoder());
	assertAccountExists(greetedAccountBefore);
	assert.deepStrictEqual(greetedAccountBefore.data.count, 0);
	assert.deepStrictEqual(greetedAccountBefore.lamports, lamports(LAMPORTS_PER_SOL));

	// When we send a greet instruction using a legacy transaction.
	const transaction = await pipe(
		createTransactionMessage({ version: "legacy" }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) =>
			appendTransactionMessageInstruction(getGreetInstruction(programAddress, greetedAddress), tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash(svm.latestBlockhashLifetime(), tx),
		(tx) => signTransactionMessageWithSigners(tx),
	);
	svm.sendTransaction(transaction);

	// Then the greeted account has 1 greet.
	const greetedAccountAfter = decodeAccount(svm.getAccount(greetedAddress), getCounterDecoder());
	assertAccountExists(greetedAccountAfter);
	assert.deepStrictEqual(greetedAccountAfter.data.count, 1);
});
