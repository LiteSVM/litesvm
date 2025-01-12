import {
	PublicKey,
	LAMPORTS_PER_SOL,
	Transaction,
	TransactionInstruction,
	VersionedTransaction,
	MessageV0,
	Keypair,
} from "@solana/web3.js";
import { helloworldProgram } from "./util";

test("versioned tx", () => {
	const [svm, programId, greetedPubkey] = helloworldProgram();
	const payer = new Keypair();
	svm.airdrop(payer.publicKey, BigInt(LAMPORTS_PER_SOL));
	const ix = new TransactionInstruction({
		keys: [{ pubkey: greetedPubkey, isSigner: false, isWritable: true }],
		programId,
		data: Buffer.from([0]),
	});
	const msg = MessageV0.compile({
		payerKey: payer.publicKey,
		instructions: [ix],
		recentBlockhash: svm.latestBlockhash(),
	});
	const tx = new VersionedTransaction(msg);
	tx.sign([payer]);
	const res = svm.sendTransaction(tx);
	const greetedAccountAfter = svm.getAccount(greetedPubkey);
	expect(greetedAccountAfter).not.toBeNull();
	expect(greetedAccountAfter?.data).toEqual(new Uint8Array([1, 0, 0, 0]));
});
