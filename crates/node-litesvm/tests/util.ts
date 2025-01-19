import { ComputeBudget, LiteSVM } from "../litesvm";
import { PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { readFileSync } from "node:fs";

export function getLamports(svm: LiteSVM, address: PublicKey): number | null {
	const acc = svm.getAccount(address);
	return acc === null ? null : acc.lamports;
}

export function helloworldProgram(
	computeMaxUnits?: bigint,
): [LiteSVM, PublicKey, PublicKey] {
	const programId = PublicKey.unique();
	const greetedPubkey = PublicKey.unique();
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
	});
	svm.addProgramFromFile(programId, "program_bytes/counter.so");
	return [svm, programId, greetedPubkey];
}

export function helloworldProgramViaSetAccount(
	computeMaxUnits?: bigint,
): [LiteSVM, PublicKey, PublicKey] {
	const programId = PublicKey.unique();
	const greetedPubkey = PublicKey.unique();
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
	});
	svm.addProgram(programId, programBytes);
	return [svm, programId, greetedPubkey];
}
