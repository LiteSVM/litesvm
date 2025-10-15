import { test } from "node:test";
import assert from "node:assert/strict";
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
import { helloworldProgramViaSetAccount } from "./kit-util";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("add program via setAccount", async () => {
  const [svm, programId, greetedPubkey] = await helloworldProgramViaSetAccount();

  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);

  const blockhash = svm.latestBlockhash() as Blockhash;

  const greetedAccountBefore = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(greetedAccountBefore, null);
  assert.deepStrictEqual(greetedAccountBefore?.data, new Uint8Array([0, 0, 0, 0]));

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

  // sendTransaction only expects the message; signer is already attached by setTransactionMessageFeePayerSigner
  await svm.sendTransaction(tx);

  const greetedAccountAfter = svm.getAccount(greetedPubkey);
  assert.notStrictEqual(greetedAccountAfter, null);
  assert.deepStrictEqual(greetedAccountAfter?.data, new Uint8Array([1, 0, 0, 0]));
});
