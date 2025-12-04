import {
	AccountRole,
	Address,
	appendTransactionMessageInstructions,
	createTransactionMessage,
	generateKeyPairSigner,
	getStructDecoder,
	getU32Decoder,
	Instruction,
	lamports,
	pipe,
	SendableTransaction,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
	Transaction,
	TransactionSigner,
	TransactionWithLifetime,
} from "@solana/kit";
import { ComputeBudget, LiteSVM } from "../litesvm";

export const LAMPORTS_PER_SOL = 1_000_000_000n;

export async function getSignedTransaction(
	svm: LiteSVM,
	payer: TransactionSigner,
	instructions?: Instruction[],
): Promise<SendableTransaction & Transaction & TransactionWithLifetime> {
	return await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) =>
			instructions
				? appendTransactionMessageInstructions(instructions, tx)
				: tx,
		(tx) =>
			setTransactionMessageLifetimeUsingBlockhash(
				svm.latestBlockhashLifetime(),
				tx,
			),
		(tx) => signTransactionMessageWithSigners(tx),
	);
}

export async function generateAddress() {
	return (await generateKeyPairSigner()).address;
}

export const setComputeUnitLimit =
	(computeUnitLimit: bigint) => (svm: LiteSVM) => {
		const computeBudget = new ComputeBudget();
		computeBudget.computeUnitLimit = computeUnitLimit;
		return svm.withComputeBudget(computeBudget);
	};

export const setHelloWorldProgram =
	(programAddress: Address) => (svm: LiteSVM) =>
		svm.addProgramFromFile(programAddress, "program_bytes/counter.so");

export const setHelloWorldAccount =
	(address: Address, programAddress: Address) => (svm: LiteSVM) => {
		const initialData = new Uint8Array([0, 0, 0, 0]);
		return svm.setAccount({
			address,
			executable: false,
			programAddress: programAddress,
			lamports: lamports(LAMPORTS_PER_SOL),
			data: initialData,
			space: BigInt(initialData.length),
		});
	};

export function getGreetInstruction(
	greetedAddress: Address,
	programAddress: Address,
): Instruction {
	return {
		accounts: [{ address: greetedAddress, role: AccountRole.WRITABLE }],
		programAddress,
		data: new Uint8Array([0]),
	};
}

export function getCounterDecoder() {
	return getStructDecoder([["count", getU32Decoder()]]);
}
