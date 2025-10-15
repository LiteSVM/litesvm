import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit, TransactionMetadata } from "litesvm";
import type { Address, Blockhash } from "@solana/kit";
import {
  AccountRole,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  generateKeyPairSigner,
} from "@solana/kit";


const LAMPORTS_PER_SOL = 1_000_000_000n;

test("spl logging", async () => {
  // Program id: just use a fresh signer address
  const programSigner = await generateKeyPairSigner();
  const programId: Address = programSigner.address;

  const svm = new LiteSVMKit();
  svm.addProgramFromFile(programId, "program_bytes/spl_example_logging.so");

  // Payer and funding
  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);

  // Random readonly account passed to the program
  const randomAccount = (await generateKeyPairSigner()).address;

  const blockhash = svm.latestBlockhash() as Blockhash;

  // Build a single-instruction transaction
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
            accounts: [{ address: randomAccount, role: AccountRole.READONLY }],
            data: new Uint8Array(), 
          },
        ],
        msg,
      ),
  );

  // Simulate first
  const simRes = await svm.simulateTransaction(tx);

  // Then send
  const sendRes = await svm.sendTransaction(tx);

  if (sendRes instanceof TransactionMetadata) {
    // Check the expected specific log entry from the send transaction
    // Use logs() method for core TransactionMetadata type
    assert.strictEqual(sendRes.logs()[1], "Program log: static string");
    
    // Verify simulation also succeeded (if it has meta, check logs match)
    if ('meta' in simRes) {
      const simMeta = simRes.meta();
      const simLogs = simMeta.logMessages; // Kit types use logMessages property
      assert.deepStrictEqual(simLogs, sendRes.logs());
    }
  } else {
    throw new Error("Unexpected tx failure");
  }
});
