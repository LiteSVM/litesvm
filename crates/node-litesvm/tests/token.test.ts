import { Address } from "@solana/kit";
import { LiteSVM } from "litesvm";
import assert from "node:assert/strict";
import { test } from "node:test";

const NATIVE_MINT =
  "So11111111111111111111111111111111111111112" as Address<"So11111111111111111111111111111111111111112">;
const NATIVE_MINT_2022 =
  "9pan9bMn5HatX4EJdBwg9VgCa7Uz5HL8N1m5D3NdXejP" as Address<"9pan9bMn5HatX4EJdBwg9VgCa7Uz5HL8N1m5D3NdXejP">;

test("create native mints", () => {
  let svm = LiteSVM.default();

  assert.ok(
    !svm.getAccount(NATIVE_MINT).exists,
    "SPL Token native mint should not exist",
  );
  assert.ok(
    !svm.getAccount(NATIVE_MINT_2022).exists,
    "Token-2022 native mint should not exist",
  );

  svm = svm.withSysvars().withDefaultPrograms().withNativeMints();

  const validateData = (data: Uint8Array, mint: Address) => {
    assert.ok(
      data.filter((x) => x !== 0).length > 0,
      `${mint} data should not be empty`,
    );
  };

  const nativeMint = svm.getAccount(NATIVE_MINT);
  assert.ok(nativeMint.exists, "SPL Token native mint should exist");
  validateData(nativeMint.data, NATIVE_MINT);
  assert.ok(nativeMint.lamports > 0, "SPL Token native mint should have lamports");

  const nativeMint2022 = svm.getAccount(NATIVE_MINT_2022);
  assert.ok(nativeMint2022.exists, "Token-2022 native mint should exist");
  validateData(nativeMint2022.data, NATIVE_MINT_2022);
  assert.ok(nativeMint2022.lamports > 0, "Token-2022 native mint should have lamports");
});
