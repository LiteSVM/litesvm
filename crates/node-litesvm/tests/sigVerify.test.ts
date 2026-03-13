import {
	assertAccountExists,
	decodeAccount,
	lamports,
	SignatureBytes,
	TransactionPartialSigner,
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

test("test sigverify", async () => {
	// Given the following addresses.
	const [fakePayerAddress, programAddress, greetedAddress] = await Promise.all([
		generateAddress(),
		generateAddress(),
		generateAddress(),
	]);

	// And the following fake payer that provides an invalid signature.
	const fakePayer: TransactionPartialSigner = {
		address: fakePayerAddress,
		signTransactions: async (transactions) =>
			transactions.map(() => ({
				// Adds an invalid signature (all zeros) for the fake payer.
				[fakePayerAddress]: new Uint8Array(64).fill(42) as SignatureBytes,
			})),
	};

	// And a LiteSVM client with sigverify disabled.
	const svm = new LiteSVM();
	svm.withSigverify(false);
	svm.airdrop(fakePayer.address, lamports(LAMPORTS_PER_SOL));
	setHelloWorldProgram(svm, programAddress);
	setHelloWorldAccount(svm, greetedAddress, programAddress);

	// And given the greeted account has 0 greets.
	const greetedAccountBefore = decodeAccount(
		svm.getAccount(greetedAddress),
		getCounterDecoder(),
	);
	assertAccountExists(greetedAccountBefore);
	assert.deepStrictEqual(greetedAccountBefore.data.count, 0);
	assert.deepStrictEqual(
		greetedAccountBefore.lamports,
		lamports(LAMPORTS_PER_SOL),
	);

	// When we send a greet instruction signed by the fake payer.
	const transaction = await getSignedTransaction(svm, fakePayer, [
		getGreetInstruction(greetedAddress, programAddress),
	]);
	const result = svm.sendTransaction(transaction);

	// Then the greeted account has 1 greet.
	const greetedAccountAfter = decodeAccount(
		svm.getAccount(greetedAddress),
		getCounterDecoder(),
	);
	assertAccountExists(greetedAccountAfter);
	assert.deepStrictEqual(greetedAccountAfter.data.count, 1);
});
