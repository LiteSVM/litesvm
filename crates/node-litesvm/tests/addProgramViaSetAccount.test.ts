import {
	assertAccountExists,
	decodeAccount,
	generateKeyPairSigner,
	lamports,
} from "@solana/kit";
import { LiteSVM } from "index";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
	generateAddress,
	getCounterDecoder,
	getGreetInstruction,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
	setHelloWorldAccount,
} from "./util";

test("add program via setAccount", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress, greetedAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
		generateAddress(),
	]);

	// And a LiteSVM client with a hello world program loaded using `addProgram`.
	const svm = new LiteSVM()
		.tap((svm) => svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL)))
		.tap(setHelloWorldAccount(greetedAddress, programAddress))
		.addProgram(programAddress, readFileSync("program_bytes/counter.so"));

	// And given the greeted account has 0 greets.
	const greetedAccountBefore = decodeAccount(
		svm.getAccount(greetedAddress),
		getCounterDecoder(),
	);
	assertAccountExists(greetedAccountBefore);
	assert.deepStrictEqual(greetedAccountBefore.data.count, 0);

	// When we send a greet instruction.
	const transaction = await getSignedTransaction(svm, payer, [
		getGreetInstruction(greetedAddress, programAddress),
	]);
	svm.sendTransaction(transaction);

	// Then the greeted account has 1 greet.
	const greetedAccountAfter = decodeAccount(
		svm.getAccount(greetedAddress),
		getCounterDecoder(),
	);
	assertAccountExists(greetedAccountAfter);
	assert.deepStrictEqual(greetedAccountAfter.data.count, 1);
});
