import { Keypair } from "@solana/web3.js";
import { LiteSVM, TransactionMetadata } from "litesvm-web3js";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

test("web3.js snapshots round trip through bytes and files", async () => {
	const svm = new LiteSVM();
	const { publicKey } = await Keypair.generate();
	assert(svm.airdrop(publicKey, 1_000_000n) instanceof TransactionMetadata);
	svm.warpToSlot(42n);
	const directory = mkdtempSync(join(tmpdir(), "litesvm-web3js-snapshot-"));
	try {
		const path = join(directory, "state.bin");
		svm.saveToFile(path);
		for (const restored of [
			LiteSVM.fromBytes(svm.toBytes()),
			LiteSVM.loadFromFile(path),
		]) {
			assert(restored instanceof LiteSVM);
			assert.deepEqual(restored.getAccount(publicKey), svm.getAccount(publicKey));
			assert.equal(restored.getClock().slot, 42n);
			assert.equal(restored.latestBlockhash(), svm.latestBlockhash());
			restored.warpToSlot(43n);
			assert.equal(svm.getClock().slot, 42n);
		}
	} finally {
		rmSync(directory, { recursive: true, force: true });
	}
	assert.throws(
		() => LiteSVM.fromBytes(new Uint8Array([255, 1, 2, 3])),
		/Failed to load snapshot/,
	);
});
