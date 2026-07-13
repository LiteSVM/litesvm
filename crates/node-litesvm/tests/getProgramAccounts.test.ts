import { EncodedAccount, lamports } from "@solana/kit";
import { LiteSVM } from "litesvm/kit";
import assert from "node:assert/strict";
import { test } from "node:test";
import { generateAddress, LAMPORTS_PER_SOL } from "./util";

test("get program accounts", async () => {
	// Given two programs, where the first owns two accounts and the second owns one.
	const [ownedA1, ownedA2, ownedB, programA, programB, unusedProgram] =
		await Promise.all([
			generateAddress(),
			generateAddress(),
			generateAddress(),
			generateAddress(),
			generateAddress(),
			generateAddress(),
		]);

	const svm = new LiteSVM();
	const accountsA: EncodedAccount[] = [ownedA1, ownedA2].map((address) => ({
		address,
		executable: false,
		lamports: lamports(LAMPORTS_PER_SOL),
		programAddress: programA,
		data: new Uint8Array([1, 2, 3, 4]),
		space: 4n,
	}));
	const accountB: EncodedAccount = {
		address: ownedB,
		executable: false,
		lamports: lamports(LAMPORTS_PER_SOL),
		programAddress: programB,
		data: new Uint8Array([9, 9]),
		space: 2n,
	};
	for (const account of [...accountsA, accountB]) {
		svm.setAccount(account);
	}

	// When we fetch the accounts owned by the first program.
	const resultA = svm.getProgramAccounts(programA);

	// Then we get back exactly the two accounts we set for it.
	assert.strictEqual(resultA.length, 2);
	assert.deepStrictEqual(
		new Set(resultA.map((account) => account.address)),
		new Set([ownedA1, ownedA2]),
	);
	for (const account of resultA) {
		assert.strictEqual(account.programAddress, programA);
		assert.deepStrictEqual(
			new Uint8Array(account.data),
			new Uint8Array([1, 2, 3, 4]),
		);
	}

	// And the second program only returns its single account.
	const resultB = svm.getProgramAccounts(programB);
	assert.strictEqual(resultB.length, 1);
	assert.strictEqual(resultB[0].address, ownedB);

	// And a program that owns nothing returns an empty list.
	assert.deepStrictEqual(svm.getProgramAccounts(unusedProgram), []);
});
