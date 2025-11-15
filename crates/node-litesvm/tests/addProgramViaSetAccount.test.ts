import { test } from "node:test";
import assert from "node:assert/strict";
import {
  generateKeyPairSigner,
  createTransactionMessage,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstructions,
  signTransactionMessageWithSigners,
  pipe,
  lamports,
  blockhash as toBlockhash, 
} from "@solana/kit";
import { helloworldProgramViaSetAccount } from "./util";

const LAMPORTS_PER_SOL = lamports(1_000_000_000n);

test("add program via setAccount", async () => {
  const [svm, programId, greetedPubkey] = await helloworldProgramViaSetAccount();
  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);
  const greetedAccountBefore = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(greetedAccountBefore, null);
  assert.deepStrictEqual(greetedAccountBefore?.data, new Uint8Array([0, 0, 0, 0]));
  assert.strictEqual(greetedAccountBefore?.owner, programId);
  assert.strictEqual(greetedAccountBefore?.lamports, LAMPORTS_PER_SOL);
  const blockhash = toBlockhash(svm.latestBlockhash());
  const ix = {
    programAddress: programId,
    accounts: [
      { address: greetedPubkey, role: 1 },
    ],
    data: new Uint8Array([0]),
  };
  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) =>
      setTransactionMessageLifetimeUsingBlockhash(
        { blockhash, lastValidBlockHeight: 0n },
        m
      ),
    (m) => appendTransactionMessageInstructions([ix], m)
  );
  const signedTx = await signTransactionMessageWithSigners(tx);
  svm.sendTransaction(signedTx);
  const greetedAccountAfter = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(greetedAccountAfter, null);
  assert.deepStrictEqual(greetedAccountAfter?.data, new Uint8Array([1, 0, 0, 0]));
});
