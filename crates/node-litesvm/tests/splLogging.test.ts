import { LiteSVM, TransactionMetadata } from "litesvm";
import {
	Keypair,
	LAMPORTS_PER_SOL,
	PublicKey,
	Transaction,
	TransactionInstruction,
} from "@solana/web3.js";

test("spl logging", () => {
	const programId = PublicKey.unique();
	const svm = new LiteSVM();
	svm.addProgramFromFile(programId, "program_bytes/spl_example_logging.so");
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const blockhash = svm.latestBlockhash();
	const ixs = [
		new TransactionInstruction({
			programId,
			keys: [
				{ pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
			],
		}),
	];
	const tx = new Transaction();
	tx.recentBlockhash = blockhash;
	tx.add(...ixs);
	tx.sign(payer);
	// let's sim it first
	const simRes = svm.simulateTransaction(tx);
	const sendRes = svm.sendTransaction(tx);
	if (sendRes instanceof TransactionMetadata) {
		expect(simRes.meta().logs()).toEqual(sendRes.logs());
		expect(sendRes.logs()[1]).toBe("Program log: static string");
	} else {
		throw new Error("Unexpected tx failure");
	}
});
