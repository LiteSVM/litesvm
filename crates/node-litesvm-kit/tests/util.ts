import { ComputeBudget, LiteSVMKit } from "../litesvm";
import { generateKeyPairSigner, type Address } from "@solana/kit";
import { readFileSync } from "node:fs";

const LAMPORTS_PER_SOL = 1_000_000_000n;

// Helper function to generate unique addresses
async function generateUniqueAddress(): Promise<Address> {
	const keyPair = await generateKeyPairSigner();
	return keyPair.address;
}

export function getLamports(svm: LiteSVMKit, address: Address): bigint | null {
	const acc = svm.getAccount(address);
	return acc === null ? null : acc.lamports;
}

export async function helloworldProgram(
	computeMaxUnits?: bigint,
): Promise<[LiteSVMKit, Address, Address]> {
	const programId = await generateUniqueAddress();
	const greetedPubkey = await generateUniqueAddress();
	let svm = new LiteSVMKit();
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
		rentEpoch: 0n,
	});
	svm.addProgramFromFile(programId, "program_bytes/counter.so");
	return [svm, programId, greetedPubkey];
}

export async function helloworldProgramViaSetAccount(
	computeMaxUnits?: bigint,
): Promise<[LiteSVMKit, Address, Address]> {
	const programId = await generateUniqueAddress();
	const greetedPubkey = await generateUniqueAddress();
	const programBytes = readFileSync("program_bytes/counter.so");
	let svm = new LiteSVMKit();
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
		rentEpoch: 0n,
	});
	svm.addProgram(programId, programBytes);
	return [svm, programId, greetedPubkey];
}