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
    type Blockhash, Address, Instruction
} from "@solana/kit";
import {
  getTransferSolInstructionDataEncoder,
  SYSTEM_PROGRAM_ADDRESS,
} from "@solana-program/system";

const LAMPORTS_PER_SOL = 1_000_000_000n;

test("one transfer", async () => {
  const svm = new LiteSVMKit();

  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, LAMPORTS_PER_SOL);

  // Create a system-owned receiver account (starts at 0 lamports)
  const receiver = await generateKeyPairSigner();
  const receiverAddress: Address = receiver.address;
  svm.setAccount(receiverAddress, {
    address: receiverAddress,
    owner: SYSTEM_PROGRAM_ADDRESS,
    executable: false,
    lamports: 0n,
    data: new Uint8Array(),
    rentEpoch: 0n,
  });

  const blockhash = svm.latestBlockhash() as Blockhash;
  const transferLamports = 1_000_000n;

  function getSolTransferInstruction(args: {
    fromAddress: Address;
    toAddress: Address;
    lamports: bigint; // u64 bigint
  }) {
    return {
      programAddress: SYSTEM_PROGRAM_ADDRESS,
      accounts: [
        { address: args.fromAddress, role: AccountRole.WRITABLE_SIGNER },
        { address: args.toAddress, role: AccountRole.WRITABLE },
      ],
      data: new Uint8Array(
        getTransferSolInstructionDataEncoder().encode({
          amount: args.lamports,
        }),
      ),
    } satisfies Instruction;
  }

  const ix = getSolTransferInstruction({
    fromAddress: payer.address,
    toAddress: receiverAddress,
    lamports: transferLamports,
  });

  const tx = pipe(
    createTransactionMessage({ version: 0 }),
    (msg) => setTransactionMessageFeePayerSigner(payer, msg),
    (msg) =>
      setTransactionMessageLifetimeUsingBlockhash(
        { blockhash, lastValidBlockHeight: 0n },
        msg,
      ),
    (msg) => appendTransactionMessageInstructions([ix], msg),
  );

  await svm.sendTransaction(tx);

  const balanceAfter = svm.getBalance(receiverAddress);
  assert.strictEqual(balanceAfter, transferLamports);
});
