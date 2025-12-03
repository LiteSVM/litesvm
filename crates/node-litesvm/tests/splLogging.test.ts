import { AccountRole, generateKeyPairSigner, lamports } from "@solana/kit";
import { LiteSVM, TransactionMetadata } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateAddress, getSignedTransaction, LAMPORTS_PER_SOL } from "./util";

test("spl logging", async () => {
	// Given the following addresses and signers.
	const [payer, programAddress, loggedAddress] = await Promise.all([
		generateKeyPairSigner(),
		generateAddress(),
		generateAddress(),
	]);

	// And a LiteSVM client with a logging program loaded from `spl_example_logging.so`.
	const svm = new LiteSVM()
		.tap((svm) => svm.airdrop(payer.address, lamports(LAMPORTS_PER_SOL)))
		.addProgramFromFile(programAddress, "program_bytes/spl_example_logging.so");

	// When we simulate and send a transaction that calls the program.
	const transaction = await getSignedTransaction(svm, payer, [
		{ accounts: [{ address: loggedAddress, role: AccountRole.READONLY }], programAddress },
	]);
	const simulationResult = svm.simulateTransaction(transaction);
	const result = svm.sendTransaction(transaction);

	// Then we expect the logs from simulation and execution to match.
	if (result instanceof TransactionMetadata) {
		assert.deepStrictEqual(simulationResult.meta().logs(), result.logs());
		assert.strictEqual(result.logs()[1], "Program log: static string");
	} else {
		throw new Error("Unexpected tx failure");
	}
});
