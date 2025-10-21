import {
  Account,
  AddressAndAccount,
  Clock,
  ComputeBudget,
  EpochRewards,
  EpochSchedule,
  FailedTransactionMetadata,
  FeatureSet,
  SimulatedTransactionInfo as SimulatedTransactionInfoInner,
  LiteSvm as LiteSVMInner,
  Rent,
  SlotHash,
  SlotHistory,
  StakeHistory,
  TransactionMetadata,
} from "./internal";

export {
  Account,
  Clock,
  ComputeBudget,
  EpochRewards,
  EpochSchedule,
  FailedTransactionMetadata,
  FeatureSet,
  InnerInstruction,
  Rent,
  SlotHash,
  SlotHistory,
  SlotHistoryCheck,
  StakeHistory,
  StakeHistoryEntry,
  TransactionMetadata,
  TransactionReturnData,
} from "./internal";

// ---- Solana Kit imports ----
import type { Address, Transaction } from "@solana/kit";
import {
  getAddressEncoder,
  getAddressDecoder,
  getTransactionEncoder,
  getCompiledTransactionMessageDecoder,
} from "@solana/kit";
// ----------------------------

// A minimal AccountInfo equivalent using Kit-friendly primitives.
export type AccountInfoBytes = {
  executable: boolean;
  owner: Address;
  lamports: bigint;
  data: Uint8Array;
  rentEpoch: bigint;
};

const addressEncoder = getAddressEncoder();
const addressDecoder = getAddressDecoder();

/** Convert internal Account -> AccountInfoBytes */
function toAccountInfo(acc: Account): AccountInfoBytes {
  const ownerAddr = addressDecoder.decode(acc.owner()); // owner() returns 32-byte pubkey
  return {
    executable: acc.executable(),
    owner: ownerAddr,
    lamports: acc.lamports(), // bigint already
    data: acc.data(),
    rentEpoch: acc.rentEpoch(), // bigint already
  };
}

/** Convert AccountInfoBytes -> internal Account */
function fromAccountInfo(acc: AccountInfoBytes): Account {
  const rentEpoch = acc.rentEpoch ?? 0n;
  const ownerBytes = addressEncoder.encode(acc.owner);
  return new Account(
    BigInt(acc.lamports),
    new Uint8Array(acc.data),
    new Uint8Array(ownerBytes),
    acc.executable,
    BigInt(rentEpoch)
  );
}

function convertAddressAndAccount(val: AddressAndAccount): [Address, Account] {
  // AddressAndAccount.address is raw 32-byte array in your internal bindings.
  const addr = addressDecoder.decode(val.address);
  return [addr, val.account()];
}

export class SimulatedTransactionInfo {
  constructor(inner: SimulatedTransactionInfoInner) {
    this.inner = inner;
  }
  private inner: SimulatedTransactionInfoInner;

  meta(): TransactionMetadata {
    return this.inner.meta();
  }
  postAccounts(): [Address, Account][] {
    return this.inner.postAccounts().map(convertAddressAndAccount);
  }
}

/**
 * LiteSVMKit wrapper using Solana Kit types.
 */
export class LiteSVMKit {
  /** Create a new LiteSVMKit instance with standard functionality enabled */
  constructor() {
    const inner = new LiteSVMInner();
    this.inner = inner;
  }
  private inner: LiteSVMInner;

  /** Create a new LiteSVMKit instance with minimal functionality enabled */
  static default(): LiteSVMKit {
    const svm = new LiteSVMKit();
    const inner = LiteSVMInner.default();
    svm.inner = inner;
    return svm;
  }

  withComputeBudget(budget: ComputeBudget): LiteSVMKit {
    this.inner.setComputeBudget(budget);
    return this;
  }
  withSigverify(sigverify: boolean): LiteSVMKit {
    this.inner.setSigverify(sigverify);
    return this;
  }
  withBlockhashCheck(check: boolean): LiteSVMKit {
    this.inner.setBlockhashCheck(check);
    return this;
  }
  withSysvars(): LiteSVMKit {
    this.inner.setSysvars();
    return this;
  }
  withFeatureSet(featureSet: FeatureSet): LiteSVMKit {
    this.inner.setFeatureSet(featureSet);
    return this;
  }
  withBuiltins(): LiteSVMKit {
    this.inner.setBuiltins();
    return this;
  }
  withLamports(lamports: bigint): LiteSVMKit {
    this.inner.setLamports(lamports);
    return this;
  }
  withDefaultPrograms(): LiteSVMKit {
    this.inner.setDefaultPrograms();
    return this;
  }
  withTransactionHistory(capacity: bigint): LiteSVMKit {
    this.inner.setTransactionHistory(capacity);
    return this;
  }
  withLogBytesLimit(limit?: bigint): LiteSVMKit {
    this.inner.setLogBytesLimit(limit);
    return this;
  }
  withPrecompiles(): LiteSVMKit {
    this.inner.setPrecompiles();
    return this;
  }

  minimumBalanceForRentExemption(dataLen: bigint): bigint {
    return this.inner.minimumBalanceForRentExemption(dataLen);
  }

  getAccount(address: Address): AccountInfoBytes | null {
    const addrBytes = addressEncoder.encode(address);
    const inner = this.inner.getAccount(new Uint8Array(addrBytes));
    return inner === null ? null : toAccountInfo(inner);
  }

  setAccount(address: Address, account: AccountInfoBytes) {
    const addrBytes = addressEncoder.encode(address);
    this.inner.setAccount(new Uint8Array(addrBytes), fromAccountInfo(account));
  }

  getBalance(address: Address): bigint | null {
    const addrBytes = addressEncoder.encode(address);
    return this.inner.getBalance(new Uint8Array(addrBytes));
  }

  latestBlockhash(): string {
    return this.inner.latestBlockhash();
  }

  getTransaction(
    signature: Uint8Array
  ): TransactionMetadata | FailedTransactionMetadata | null {
    return this.inner.getTransaction(signature);
  }

  airdrop(
    address: Address,
    lamports: bigint
  ): TransactionMetadata | FailedTransactionMetadata | null {
    return this.inner.airdrop(new Uint8Array(addressEncoder.encode(address)), lamports);
  }

  addProgramFromFile(programId: Address, path: string) {
    return this.inner.addProgramFromFile(
      new Uint8Array(addressEncoder.encode(programId)),
      path
    );
  }

  addProgram(programId: Address, programBytes: Uint8Array) {
    return this.inner.addProgram(
      new Uint8Array(addressEncoder.encode(programId)),
      programBytes
    );
  }

  /**
   * Process a signed transaction (Kit `Transaction`).
   * Uses message version to route to legacy vs. v0 path.
   */
  sendTransaction(
    tx: Transaction
  ): TransactionMetadata | FailedTransactionMetadata {
    const internal = this.inner;

    // Decide legacy vs. v0 by decoding the compiled message.
    const compiled = getCompiledTransactionMessageDecoder().decode(
      tx.messageBytes
    ); // has .version
    const wireBytes = getTransactionEncoder().encode(tx); // raw wire bytes ready to run :contentReference[oaicite:1]{index=1}

    if (compiled.version === "legacy") {
      return internal.sendLegacyTransaction(new Uint8Array(wireBytes));
    } else {
      return internal.sendVersionedTransaction(new Uint8Array(wireBytes));
    }
  }

  /**
   * Simulate a signed transaction (Kit `Transaction`).
   */
  simulateTransaction(
    tx: Transaction
  ): FailedTransactionMetadata | SimulatedTransactionInfo {
    const internal = this.inner;
    const compiled = getCompiledTransactionMessageDecoder().decode(
      tx.messageBytes
    );
    const wireBytes = getTransactionEncoder().encode(tx); // :contentReference[oaicite:2]{index=2}
    const inner =
      compiled.version === "legacy"
        ? internal.simulateLegacyTransaction(new Uint8Array(wireBytes))
        : internal.simulateVersionedTransaction(new Uint8Array(wireBytes));

    return inner instanceof FailedTransactionMetadata
      ? inner
      : new SimulatedTransactionInfo(inner);
  }

  expireBlockhash() {
    this.inner.expireBlockhash();
  }
  warpToSlot(slot: bigint) {
    this.inner.warpToSlot(slot);
  }
  getClock(): Clock {
    return this.inner.getClock();
  }
  setClock(clock: Clock) {
    this.inner.setClock(clock);
  }
  getEpochRewards(): EpochRewards {
    return this.inner.getEpochRewards();
  }
  setEpochRewards(rewards: EpochRewards) {
    this.inner.setEpochRewards(rewards);
  }
  getEpochSchedule(): EpochSchedule {
    return this.inner.getEpochSchedule();
  }
  setEpochSchedule(schedule: EpochSchedule) {
    this.inner.setEpochSchedule(schedule);
  }
  getLastRestartSlot(): bigint {
    return this.inner.getLastRestartSlot();
  }
  setLastRestartSlot(slot: bigint) {
    this.inner.setLastRestartSlot(slot);
  }
  getRent(): Rent {
    return this.inner.getRent();
  }
  setRent(rent: Rent) {
    this.inner.setRent(rent);
  }
  getSlotHashes(): SlotHash[] {
    return this.inner.getSlotHashes();
  }
  setSlotHashes(hashes: SlotHash[]) {
    this.inner.setSlotHashes(hashes);
  }
  getSlotHistory(): SlotHistory {
    return this.inner.getSlotHistory();
  }
  setSlotHistory(history: SlotHistory) {
    this.inner.setSlotHistory(history);
  }
  getStakeHistory(): StakeHistory {
    return this.inner.getStakeHistory();
  }
  setStakeHistory(history: StakeHistory) {
    this.inner.setStakeHistory(history);
  }
}
