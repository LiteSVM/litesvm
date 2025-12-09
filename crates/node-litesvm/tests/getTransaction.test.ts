import {
	generateKeyPairSigner,
	getSignatureFromTransaction,
	lamports,
} from "@solana/kit";
import { TransactionMetadata } from "internal";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
	generateAddress,
	getGreetInstruction,
	getSignedTransaction,
	LAMPORTS_PER_SOL,
	setHelloWorldAccount,
	setHelloWorldProgram,
} from "./util";
import { LiteSVM } from "index";

test("get transaction", async () => {
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

	// And given we have sent a greet transaction.
	const transaction = await getSignedTransaction(svm, payer, [
		getGreetInstruction(greetedAddress, programAddress),
	]);
	svm.sendTransaction(transaction);

	// When we fetch the transaction by its signature.
	const signature = getSignatureFromTransaction(transaction);
	const fetched = svm.getTransaction(signature);

	// Then we expect to get its transaction metadata.
	assert.ok(fetched instanceof TransactionMetadata);
});
