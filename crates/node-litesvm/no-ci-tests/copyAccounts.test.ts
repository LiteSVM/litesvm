import { test } from "node:test";
import assert from "node:assert/strict";
import { LiteSVM, AccountInfoBytes } from "../litesvm";
import { address, createSolanaRpc, fetchJsonParsedAccount, lamports } from "@solana/kit";

test("copy accounts from devnet", async () => {
    const usdcMint = address("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    const rpc = createSolanaRpc("https://api.devnet.solana.com");
    const accountInfo = await fetchJsonParsedAccount(rpc, usdcMint);
    const svm = new LiteSVM();
    if ("exists" in accountInfo && !accountInfo.exists) {
        assert.fail("Account does not exist on devnet");
        return;
    }
    const account = accountInfo as any;
    const dataArray = Array.isArray(account.data) 
        ? new Uint8Array(account.data) 
        : new Uint8Array(Object.values(account.data));

    const accountToSet: AccountInfoBytes = {
        executable: Boolean(account.executable),
        owner: account.owner || usdcMint,
        lamports: lamports(BigInt(account.lamports || 0)),
        data: dataArray,
        space: BigInt(dataArray.length),
    };
    
    svm.setAccount(usdcMint, accountToSet);
    const rawAccount = svm.getAccount(usdcMint);
    assert.notStrictEqual(rawAccount, null);
});