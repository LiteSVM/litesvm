import { ComputeBudget, LiteSVM } from "../litesvm";
import { lamports, generateKeyPairSigner, Address } from "@solana/kit";
import { readFileSync } from "node:fs";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

export function getLamports(svm: LiteSVM, address: Address): bigint | null {
	const acc = svm.getAccount(address);
	return acc === null ? null : acc.lamports;
}

export async function helloworldProgram(
	computeMaxUnits?: bigint,
): Promise<[LiteSVM, Address, Address]> {
	const programSigner = await generateKeyPairSigner();
	const greetedSigner = await generateKeyPairSigner();
	const programId = programSigner.address;
	const greetedPubkey = greetedSigner.address;
	let svm = new LiteSVM();
	if (computeMaxUnits) {
		const computeBudget = new ComputeBudget();
		computeBudget.computeUnitLimit = computeMaxUnits;
		svm = svm.withComputeBudget(computeBudget);
	}
	svm.setAccount(greetedPubkey, {
		executable: false,
		owner: programId,
		lamports: LAMPORTS_PER_SOL,
		data: new Uint8Array([0, 0, 0, 0]),
		space: BigInt(4),
	});
	svm.addProgramFromFile(programId, "program_bytes/counter.so");
	return [svm, programId, greetedPubkey];
}

export async function helloworldProgramViaSetAccount(
	computeMaxUnits?: bigint,
): Promise<[LiteSVM, Address, Address]> {
	const programSigner = await generateKeyPairSigner();
	const greetedSigner = await generateKeyPairSigner();
	const programId = programSigner.address;
	const greetedPubkey = greetedSigner.address;
	const programBytes = readFileSync("program_bytes/counter.so");
	let svm = new LiteSVM();
	if (computeMaxUnits) {
		const computeBudget = new ComputeBudget();
		computeBudget.computeUnitLimit = computeMaxUnits;
		svm = svm.withComputeBudget(computeBudget);
	}
	svm.setAccount(greetedPubkey, {
		executable: false,
		owner: programId,
		lamports: LAMPORTS_PER_SOL,
		data: new Uint8Array([0, 0, 0, 0]),
		space: BigInt(4),
	});
	svm.addProgram(programId, programBytes);
	return [svm, programId, greetedPubkey];
}