console.log("In clock module");
import { test } from "node:test";
import assert from "node:assert/strict";
console.log("Doing litesvm imports");
import {
  FailedTransactionMetadata,
  LiteSVMKit,
  TransactionMetadata,
} from "litesvm";
console.log("Doing kit imports");
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
import { SYSVAR_CLOCK_ADDRESS } from "@solana/sysvars";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("clock", async () => {
  console.log("Running clock test");
  const programSigner = await generateKeyPairSigner();
  const programId = programSigner.address;
  console.log("Calling new LiteSVMKit()");
  const svm = new LiteSVMKit();
  console.log("Calling addProgramFromFile");
  svm.addProgramFromFile(programId, "program_bytes/litesvm_clock_example.so");
  console.log("Calling generateKeyPairSigner");
  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);
  const blockhash = svm.latestBlockhash() as Blockhash;

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
            accounts: [
              { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY }, // pass Clock
            ],
            data: new Uint8Array(),
          },
        ],
        msg,
      ),
  );

  // Set time to a "future" moment; 1735689600n == Jan 1, 2025 UTC
  const initialClock = svm.getClock();
  initialClock.unixTimestamp = 1735689600n;
  svm.setClock(initialClock);

  // This should fail because the program expects ~Jan 1970-ish
  const failed = await svm.sendTransaction(tx);
  if (failed instanceof FailedTransactionMetadata) {
    assert.ok(failed.err().toString().includes("ProgramFailedToComplete"));
  } else {
    throw new Error("Expected transaction failure here");
  }

  // Turn back time
  const newClock = svm.getClock();
  newClock.unixTimestamp = 50n; // near epoch
  svm.setClock(newClock);

  const tx2 = pipe(
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
            accounts: [
              { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY }, // pass Clock
            ],
            data: new Uint8Array(Buffer.from("foobar")),
          },
        ],
        msg,
      ),
  );

  // Now it should succeed
  const success = await svm.sendTransaction(tx2);
  assert.ok(success instanceof TransactionMetadata);
  console.log("Finished clock test");
});
