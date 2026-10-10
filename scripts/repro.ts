import BigNumber from "bignumber.js";
import { createRequire } from "module";
import { readFileSync } from "fs";
import {
  MINT_SIZE,
  MintLayout,
} from "@solana/spl-token";
import {
  appendTransactionMessageInstruction,
  createTransactionMessage,
  generateKeyPairSigner,
  lamports,
  setTransactionMessageFeePayerSigner,
  signTransactionMessageWithSigners,
  type Address,
  type KeyPairSigner,
} from "@solana/kit";
import {
  findAssociatedTokenPda,
  getCreateAssociatedTokenInstruction,
  TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import {
  PublicKey,
} from "@solana/web3.js";
import {
  FeatureSet,
  LiteSVM,
} from "litesvm";

// If "send" or "simulate" this script will hang. Set this to "skip" to avoid sending the tx: this
// script will complete and will not hang.
const MODE = process.env.MODE ?? "send";
const SOME_CONST = 6;
const PAYER_LAMPORTS = 1_000_000_000_000n;
const require = createRequire(import.meta.url);
const ACCOUNT_DATA_DIRECT_MAPPING =
  "6f2qai82RU7Dutj1WJfRzLJKYA36QWvTa89CR1imgj7N";

const createMintData = (mintAuthority: PublicKey): Buffer => {
  const data = Buffer.alloc(MINT_SIZE);
  MintLayout.encode(
    {
      mintAuthorityOption: 1,
      mintAuthority,
      supply: 0n,
      decimals: SOME_CONST,
      isInitialized: true,
      freezeAuthorityOption: 0,
      freezeAuthority: PublicKey.default,
    },
    data,
  );
  return data;
};

const svm = new LiteSVM()
  .withLamports(PAYER_LAMPORTS * 2n)
  .withTransactionHistory(0n)
  .withLogBytesLimit(undefined);

// if (process.env.FEATURE_SOURCE === "repo") {
//   const featureSet = new FeatureSet();
//   const featuresSource = readFileSync("../crates/litesvm/src/features.rs", "utf8");
//   for (const match of featuresSource.matchAll(/address!\("([^"]+)"\)/g)) {
//     featureSet.activate(new PublicKey(match[1]).toBytes(), 0n);
//   }
//   for (const feature of (process.env.EXTRA_FEATURES ?? "").split(",")) {
//     if (feature.length > 0) {
//       featureSet.activate(new PublicKey(feature).toBytes(), 0n);
//     }
//   }
//   if (process.env.NO_DIRECT_MAPPING === "1") {
//     featureSet.deactivate(new PublicKey(ACCOUNT_DATA_DIRECT_MAPPING).toBytes());
//   }
//   svm.withFeatureSet(featureSet);
// } else if (process.env.ALL_FEATURES === "1" || process.env.NO_DIRECT_MAPPING === "1") {
//   const featureSet = FeatureSet.allEnabled();
//   if (process.env.NO_DIRECT_MAPPING === "1") {
//     featureSet.deactivate(new PublicKey(ACCOUNT_DATA_DIRECT_MAPPING).toBytes());
//   }
//   svm.withFeatureSet(featureSet);
// }

const payer: KeyPairSigner = await generateKeyPairSigner();
const owner = await generateKeyPairSigner();
const mint = await generateKeyPairSigner();
const [associatedTokenAccount] = await findAssociatedTokenPda({
  owner: owner.address,
  mint: mint.address,
  tokenProgram: TOKEN_PROGRAM_ADDRESS,
});

svm.airdrop(payer.address, lamports(PAYER_LAMPORTS));
svm.setAccount({
  address: mint.address,
  executable: false,
  programAddress: TOKEN_PROGRAM_ADDRESS,
  lamports: lamports(9999999n),
  data: createMintData(PublicKey.default),
  space: 0n,
});

// Note: a simple transfer or various other kinds of ix will not cause the hang: creating an ATA is
// the only example noted so far.
const message = appendTransactionMessageInstruction(
  getCreateAssociatedTokenInstruction({
    payer,
    ata: associatedTokenAccount,
    owner: owner.address,
    mint: mint.address,
  }),
  svm.setTransactionMessageLifetimeUsingLatestBlockhash(
    setTransactionMessageFeePayerSigner(
      payer,
      createTransactionMessage({ version: 0 }),
    ),
  ),
);
const tx = await signTransactionMessageWithSigners(message);

console.log(`[bnstall] before ATA ${MODE}`);
// Note: Send OR simulate BOTH TRIGGER the bug. 
if (MODE === "send") {
  svm.sendTransaction(tx);
} else if (MODE === "simulate") {
  svm.simulateTransaction(tx);
} else if (MODE !== "skip") {
  throw new Error(`Unknown MODE: ${MODE}`);
}
console.log(`[bnstall] after ATA ${MODE}`);

console.log("[bnstall] node", process.version);
console.log("[bnstall] bignumber path", require.resolve("bignumber.js"));
console.log(
  "[bnstall] bignumber version",
  require("bignumber.js/package.json").version,
);
console.log("[bnstall] BigNumber config", BigNumber.config());

console.log("[bnstall] before math");
// Hangs here (or on any BigNumber div or various other number operations)
const out = new BigNumber("100000000").div("1000000");
console.log("[bnstall] after math", out.toString());
