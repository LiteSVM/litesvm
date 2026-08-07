import { getCreateAccountInstruction } from "@solana-program/system";
import {
	getCreateAssociatedTokenIdempotentInstructionAsync,
	getInitializeMint2Instruction,
	TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import { generateKeyPairSigner, lamports } from "@solana/kit";
import assert from "node:assert/strict";
import { test } from "node:test";
import { FailedTransactionMetadata, LiteSVM } from "../litesvm";
import { getSignedTransaction, LAMPORTS_PER_SOL } from "./util";

// Not inlined, so V8 cannot constant-fold the modulo away: it must emit the
// fld/fprem sequence it uses for `%` on heap doubles.
function mod(a: number, b: number): number {
	return a % b;
}

// Regression test for https://github.com/LiteSVM/litesvm/issues/396.
// The SBPF JIT spills a scratch register into MM0 on the `callx` path and never issues
// EMMS, so it used to return with the whole x87 tag word marked valid. The next `%` V8
// evaluated on a heap double then overflowed the FPU stack and yielded NaN, once.
test("x87 state is clean after a transaction making an indirect call", async () => {
	const svm = new LiteSVM();
	const [payer, mint] = await Promise.all([
		generateKeyPairSigner(),
		generateKeyPairSigner(),
	]);
	svm.airdrop(payer.address, lamports(1000n * LAMPORTS_PER_SOL));

	const instructions = [
		getCreateAccountInstruction({
			payer,
			newAccount: mint,
			lamports: svm.minimumBalanceForRentExemption(82n),
			space: 82n,
			programAddress: TOKEN_PROGRAM_ADDRESS,
		}),
		getInitializeMint2Instruction({
			mint: mint.address,
			decimals: 6,
			mintAuthority: payer.address,
		}),
		await getCreateAssociatedTokenIdempotentInstructionAsync({
			payer,
			owner: payer.address,
			mint: mint.address,
		}),
	];

	const result = svm.sendTransaction(
		await getSignedTransaction(svm, payer, instructions),
	);
	assert.ok(
		!(result instanceof FailedTransactionMetadata),
		`transaction failed: ${result}`,
	);

	assert.ok(Object.is(mod(-0, 64), -0), "`-0 % 64` should be -0, got NaN");
});
