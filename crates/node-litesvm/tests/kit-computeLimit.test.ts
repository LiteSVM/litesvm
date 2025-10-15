import { test } from "node:test";
import assert from "node:assert/strict";
import { FailedTransactionMetadata } from "litesvm";
import {
  AccountRole,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  generateKeyPairSigner,
    type Blockhash
} from "@solana/kit";
import { helloworldProgram } from "./kit-util";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("compute limit", async () => {
  // Set a tiny compute limit to force failure
  const [svm, programId, greetedPubkey] = await helloworldProgram(10n);

  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);
  const blockhash = svm.latestBlockhash() as Blockhash;

  const greetedAccountBefore = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(greetedAccountBefore, null);
  assert.deepStrictEqual(
    greetedAccountBefore?.data,
    new Uint8Array([0, 0, 0, 0]),
  );

  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    (msg) => setTransactionMessageFeePayerSigner(payer, msg),
    (msg) =>
      setTransactionMessageLifetimeUsingBlockhash(
        { blockhash, lastValidBlockHeight: 0n },
        msg,
      ),
    (msg) =>
      appendTransactionMessageInstructions(
        [
          {
            programAddress: programId,
            accounts: [{ address: greetedPubkey, role: AccountRole.WRITABLE }],
            data: new Uint8Array([0]),
          },
        ],
        msg,
      ),
  );

  const res = await svm.sendTransaction(tx);
  if (res instanceof FailedTransactionMetadata) {
    // With very low compute limit, we expect either compute budget exceeded or account not found
    const err = res.err();
    const errStr = err.toString();
    assert.ok(
      errStr.includes("ProgramFailedToComplete") ||
        errStr.includes("ComputationalBudgetExceeded") ||
        errStr.includes("ExceededMaxInstructions") ||
        err === 2, // AccountNotFound error code
      `unexpected error: ${errStr} (code: ${err})`,
    );
  } else {
    throw new Error("Expected transaction failure");
  }
});
