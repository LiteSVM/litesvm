// tests/kit-util.ts
import { ComputeBudget, LiteSVMKit } from "../litesvm";
import { Address } from "@solana/kit";
import { generateKeyPairSigner } from "@solana/signers";
import { readFileSync } from "node:fs";

// Use bigint for lamports/rentEpoch to match KitAccountInfo types.
const ONE_SOL = 1_000_000_000n;

function toNumber(v?: number | bigint): number | undefined {
  return v === undefined ? undefined : (typeof v === "bigint" ? Number(v) : v);
}

export function getLamports(svm: LiteSVMKit, addr: Address): bigint | null {
  const acc = svm.getAccount(addr);
  return acc === null ? null : acc.lamports; // bigint
}

export async function helloworldProgram(
  computeMaxUnits?: number | bigint,
): Promise<[LiteSVMKit, Address, Address]> {
  const programSigner = await generateKeyPairSigner();
  const greetedSigner = await generateKeyPairSigner();

  const programId: Address = programSigner.address;
  const greetedPubkey: Address = greetedSigner.address;

  let svm = new LiteSVMKit();
  const units = toNumber(computeMaxUnits);
  if (units !== undefined) {
    const computeBudget = new ComputeBudget();
    computeBudget.computeUnitLimit = BigInt(units);
    svm = svm.withComputeBudget(computeBudget);
  }

  // NOTE: two-arg form: setAccount(address, account)
  svm.setAccount(greetedPubkey, {
    address: greetedPubkey,
    executable: false,
    owner: programId,
    lamports: ONE_SOL,                  // bigint
    data: new Uint8Array([0, 0, 0, 0]),
    rentEpoch: 0n,                      // bigint
  });

  svm.addProgramFromFile(programId, "program_bytes/counter.so");
  return [svm, programId, greetedPubkey];
}

export async function helloworldProgramViaSetAccount(
  computeMaxUnits?: number | bigint,
): Promise<[LiteSVMKit, Address, Address]> {
  const programSigner = await generateKeyPairSigner();
  const greetedSigner = await generateKeyPairSigner();

  const programId: Address = programSigner.address;
  const greetedPubkey: Address = greetedSigner.address;
  const programBytes = readFileSync("program_bytes/counter.so");

  let svm = new LiteSVMKit();
  const units = toNumber(computeMaxUnits);
  if (units !== undefined) {
    const computeBudget = new ComputeBudget();
    computeBudget.computeUnitLimit = BigInt(units);
    svm = svm.withComputeBudget(computeBudget);
  }

  svm.setAccount(greetedPubkey, {
    address: greetedPubkey,
    executable: false,
    owner: programId,
    lamports: ONE_SOL,                  // bigint
    data: new Uint8Array([0, 0, 0, 0]),
    rentEpoch: 0n,                      // bigint
  });

  svm.addProgram(programId, programBytes);
  return [svm, programId, greetedPubkey];
}
