import { ComputeBudget, LiteSVM } from "../litesvm";
import {
	Address,
	generateKeyPairSigner,
	lamports,
	Lamports,
} from "@solana/kit";
import { readFileSync } from "node:fs";

export const LAMPORTS_PER_SOL = 1_000_000_000n;

export async function generateAddress() {
	return (await generateKeyPairSigner()).address;
}

export function getLamports(svm: LiteSVM, address: Address): Lamports | null {
	const acc = svm.getAccount(address);
	return acc === null ? null : acc.lamports;
}

export async function helloworldProgram(
	computeMaxUnits?: bigint,
): Promise<[LiteSVM, Address, Address]> {
	const [programAddress, greetedAddress] = await Promise.all([
		generateAddress(),
		generateAddress(),
	]);
	let svm = new LiteSVM();
	if (computeMaxUnits) {
		const computeBudget = new ComputeBudget();
		computeBudget.computeUnitLimit = computeMaxUnits;
		svm = svm.withComputeBudget(computeBudget);
	}
	const data = new Uint8Array([0, 0, 0, 0]);
	svm.setAccount(greetedAddress, {
		executable: false,
		programAddress: programAddress,
		lamports: lamports(1000000000n),
		data,
		space: BigInt(data.length),
	});
	svm.addProgramFromFile(programAddress, "program_bytes/counter.so");
	return [svm, programAddress, greetedAddress];
}

/* TODO: Combine helpers whilst testing both functions. I.e. unwrap second one. */
export async function helloworldProgramViaSetAccount(
	computeMaxUnits?: bigint,
): Promise<[LiteSVM, Address, Address]> {
	const [programAddress, greetedAddress] = await Promise.all([
		generateAddress(),
		generateAddress(),
	]);
	let svm = new LiteSVM();
	if (computeMaxUnits) {
		const computeBudget = new ComputeBudget();
		computeBudget.computeUnitLimit = computeMaxUnits;
		svm = svm.withComputeBudget(computeBudget);
	}
	const data = new Uint8Array([0, 0, 0, 0]);
	svm.setAccount(greetedAddress, {
		executable: false,
		programAddress,
		lamports: lamports(1000000000n),
		data,
		space: BigInt(data.length),
	});
	const programBytes = readFileSync("program_bytes/counter.so");
	svm.addProgram(programAddress, programBytes);
	return [svm, programAddress, greetedAddress];
}
