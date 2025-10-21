import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit, TransactionMetadata } from "litesvm";
import {
	generateKeyPairSigner,
	createTransactionMessage,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	appendTransactionMessageInstructions,
	signTransactionMessageWithSigners,
	pipe,
	blockhash,
} from "@solana/kit";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("spl logging", async () => {
	const programId = await generateKeyPairSigner();
	const svm = new LiteSVMKit();
	svm.addProgramFromFile(programId.address, "program_bytes/spl_example_logging.so");
	
	const payer = await generateKeyPairSigner();
	svm.airdrop(payer.address, LAMPORTS_PER_SOL);
	
	// Verify program is loaded
	const programAccount = svm.getAccount(programId.address);
	assert.notStrictEqual(programAccount, null);
	assert.strictEqual(programAccount?.executable, true);
	
	// Verify payer has funds
	const payerBalance = svm.getBalance(payer.address);
	assert.strictEqual(payerBalance, LAMPORTS_PER_SOL);
	
	const latestBlockhash = blockhash(svm.latestBlockhash());
	const dummyAccount = await generateKeyPairSigner();
	
	// Create an instruction to call the logging program
	const instruction = {
		programAddress: programId.address,
		accounts: [
			{
				address: dummyAccount.address,
				role: 0, // READONLY
			},
		],
		data: new Uint8Array([]), // No data needed for basic logging
	};
	
	const transactionMessage = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: latestBlockhash, lastValidBlockHeight: 0n }, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
	);
	
	const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
	
	// Test both simulation and execution to compare logs
	const simRes = svm.simulateTransaction(signedTransaction);
	const sendRes = svm.sendTransaction(signedTransaction);
	
	if (sendRes instanceof TransactionMetadata) {
		// Both simulation and execution should succeed
		assert.deepStrictEqual(simRes.meta().logs(), sendRes.logs(), "Simulation and execution logs should match");
		
		// The logging program should produce some log output
		const logs = sendRes.logs();
		assert.ok(logs.length > 0, "Program should produce log output");
		
		// Check for expected log message from the SPL logging example
		const hasStaticStringLog = logs.some(log => log.includes("Program log: static string"));
		assert.ok(hasStaticStringLog, "Should contain 'static string' log message");
	} else {
		throw new Error("Expected transaction to succeed for logging test");
	}
});
