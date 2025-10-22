import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVMKit } from "litesvm";
import { createSolanaRpc, devnet, address, type Address } from "@solana/kit";

type AccountInfoBytes = {
  executable: boolean;
  owner: Address;
  lamports: bigint;
  data: Uint8Array;
  rentEpoch: bigint;
};

function toAccountInfoBytes(ai: {
  executable: boolean;
  owner: string; // base58
  lamports: bigint | number;
  data: [string, "base64"];
}) : AccountInfoBytes {
  return {
    executable: ai.executable,
    owner: address(ai.owner),
    lamports: BigInt(ai.lamports),
    data: Buffer.from(ai.data[0], "base64"),
    rentEpoch: 0n,
  };
}

test("copy accounts from devnet", async () => {
  const acct = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
  const rpc = createSolanaRpc(devnet("https://api.devnet.solana.com"));
  const { value } = await rpc.getAccountInfo(acct, { encoding: "base64" }).send();
  if (!value) throw new Error("Account not found on devnet.");

  const svm = new LiteSVMKit();
  svm.setAccount(acct, toAccountInfoBytes(value));

  const rawAccount = svm.getAccount(acct);
  assert.notStrictEqual(rawAccount, null);
});
