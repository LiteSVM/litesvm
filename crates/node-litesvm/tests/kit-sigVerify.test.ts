import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
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

test("test sigverify", async () => {
  // Set up program + greeted account
  let [svm, programId, greetedPubkey] = await helloworldProgram();

  // Disable signature verification in the VM
  svm = svm.withSigverify(false) as LiteSVMKit;

  // Use a real TransactionSigner for fee payer (simplest way to satisfy types)
  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);

  const blockhash = svm.latestBlockhash() as Blockhash;

  const before = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(before, null);
  assert.deepStrictEqual(before?.data, new Uint8Array([0, 0, 0, 0]));

  // Build and send a simple invoke of the program (writes greeted account)
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

  await svm.sendTransaction(tx);

  const after = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(after, null);
  assert.deepStrictEqual(after?.data, new Uint8Array([1, 0, 0, 0]));
});
