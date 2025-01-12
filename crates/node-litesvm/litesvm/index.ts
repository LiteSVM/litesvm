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
	SlotHistoryCheck,
	StakeHistory,
	StakeHistoryEntry,
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
import {
	AccountInfo,
	PublicKey,
	Transaction,
	VersionedTransaction,
} from "@solana/web3.js";

export type AccountInfoBytes = AccountInfo<Uint8Array>;

function toAccountInfo(acc: Account): AccountInfoBytes {
	const owner = new PublicKey(acc.owner());
	return {
		executable: acc.executable(),
		owner,
		lamports: Number(acc.lamports()),
		data: acc.data(),
		rentEpoch: Number(acc.rentEpoch()),
	};
}

function fromAccountInfo(acc: AccountInfoBytes): Account {
	const maybeRentEpoch = acc.rentEpoch;
	const rentEpoch = maybeRentEpoch || 0;
	return new Account(
		BigInt(acc.lamports),
		acc.data,
		acc.owner.toBytes(),
		acc.executable,
		BigInt(rentEpoch),
	);
}

function convertAddressAndAccount(
	val: AddressAndAccount,
): [PublicKey, Account] {
	return [new PublicKey(val.address), val.account()];
}

export class SimulatedTransactionInfo {
	constructor(inner: SimulatedTransactionInfoInner) {
		this.inner = inner;
	}
	private inner: SimulatedTransactionInfoInner;
	meta(): TransactionMetadata {
		return this.inner.meta();
	}
	postAccounts(): [PublicKey, Account][] {
		return this.inner.postAccounts().map(convertAddressAndAccount);
	}
}

export class LiteSVM {
	constructor() {
		const inner = new LiteSVMInner();
		this.inner = inner;
	}
	private inner: LiteSVMInner;

	withComputeBudget(budget: ComputeBudget): LiteSVM {
		this.inner.setComputeBudget(budget);
		return this;
	}

	withSigverify(sigverify: boolean): LiteSVM {
		this.inner.setSigverify(sigverify);
		return this;
	}

	withBlockhashCheck(check: boolean): LiteSVM {
		this.inner.setBlockhashCheck(check);
		return this;
	}

	withSysvars(): LiteSVM {
		this.inner.setSysvars();
		return this;
	}

	withBuiltins(featureSet?: FeatureSet): LiteSVM {
		this.inner.setBuiltins(featureSet);
		return this;
	}

	withLamports(lamports: bigint): LiteSVM {
		this.inner.setLamports(lamports);
		return this;
	}

	withSplPrograms(): LiteSVM {
		this.inner.setSplPrograms();
		return this;
	}

	withTransactionHistory(capacity: bigint): LiteSVM {
		this.inner.setTransactionHistory(capacity);
		return this;
	}

	withLogBytesLimit(limit?: bigint): LiteSVM {
		this.inner.setLogBytesLimit(limit);
		return this;
	}

	withPrecompiles(featureSet?: FeatureSet): LiteSVM {
		this.inner.setPrecompiles(featureSet);
		return this;
	}

	minimumBalanceForRentExemption(dataLen: bigint): bigint {
		return this.inner.minimumBalanceForRentExemption(dataLen);
	}

	/**
	 * Return the account at the given address.
	 * If the account is not found, None is returned.
	 * @param address - The account address to look up.
	 * @returns The account object, if the account exists.
	 */
	getAccount(address: PublicKey): AccountInfoBytes | null {
		const inner = this.inner.getAccount(address.toBytes());
		return inner === null ? null : toAccountInfo(inner);
	}

	/**
	 * Create or overwrite an account, subverting normal runtime checks.
	 *
	 * This method exists to make it easier to set up artificial situations
	 * that would be difficult to replicate by sending individual transactions.
	 * Beware that it can be used to create states that would not be reachable
	 * by sending transactions!
	 *
	 * @param address - The address to write to.
	 * @param account - The account object to write.
	 */
	setAccount(address: PublicKey, account: AccountInfoBytes) {
		this.inner.setAccount(address.toBytes(), fromAccountInfo(account));
	}

	getBalance(address: PublicKey): bigint | null {
		return this.inner.getBalance(address.toBytes());
	}

	latestBlockhash(): string {
		return this.inner.latestBlockhash();
	}

	getTransaction(
		signature: Uint8Array,
	): TransactionMetadata | FailedTransactionMetadata | null {
		return this.inner.getTransaction(signature);
	}

	airdrop(
		address: PublicKey,
		lamports: bigint,
	): TransactionMetadata | FailedTransactionMetadata | null {
		return this.inner.airdrop(address.toBytes(), lamports);
	}

	addProgramFromFile(programId: PublicKey, path: string) {
		return this.inner.addProgramFromFile(programId.toBytes(), path);
	}

	addProgram(programId: PublicKey, programBytes: Uint8Array) {
		return this.inner.addProgram(programId.toBytes(), programBytes);
	}

	sendTransaction(
		tx: Transaction | VersionedTransaction,
	): TransactionMetadata | FailedTransactionMetadata {
		const serialized = tx.serialize();
		const internal = this.inner;
		if (tx instanceof Transaction) {
			return internal.sendLegacyTransaction(serialized);
		} else {
			return internal.sendVersionedTransaction(serialized);
		}
	}

	simulateTransaction(
		tx: Transaction | VersionedTransaction,
	): FailedTransactionMetadata | SimulatedTransactionInfo {
		const serialized = tx.serialize();
		const internal = this.inner;
		const inner =
			tx instanceof Transaction
				? internal.simulateLegacyTransaction(serialized)
				: internal.simulateVersionedTransaction(serialized);
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

	/**
	 * Get the cluster clock.
	 * @returns the clock object.
	 */
	getClock(): Clock {
		return this.inner.getClock();
	}

	/**
	 * Overwrite the clock sysvar.
	 * @param clock - The clock object.
	 */
	setClock(clock: Clock) {
		this.inner.setClock(clock);
	}

	/**
	 * Get the EpochRewards sysvar.
	 * @returns the EpochRewards object.
	 */
	getEpochRewards(): EpochRewards {
		return this.inner.getEpochRewards();
	}

	/**
	 * Overwrite the EpochRewards sysvar.
	 * @param rewards - The EpochRewards object.
	 */
	setEpochRewards(rewards: EpochRewards) {
		this.inner.setEpochRewards(rewards);
	}

	/**
	 * Get the EpochSchedule sysvar.
	 * @returns the EpochSchedule object.
	 */
	getEpochSchedule(): EpochSchedule {
		return this.inner.getEpochSchedule();
	}

	/**
	 * Overwrite the EpochSchedule sysvar.
	 * @param schedule - The EpochSchedule object.
	 */
	setEpochSchedule(schedule: EpochSchedule) {
		this.inner.setEpochSchedule(schedule);
	}

	/**
	 * Get the last restart slot sysvar.
	 * @returns the last restart slot.
	 */
	getLastRestartSlot(): bigint {
		return this.inner.getLastRestartSlot();
	}

	/**
	 * Overwrite the last restart slot sysvar.
	 * @param slot - The last restart slot.
	 */
	setLastRestartSlot(slot: bigint) {
		this.inner.setLastRestartSlot(slot);
	}

	/**
	 * Get the cluster rent.
	 * @returns The rent object.
	 */
	getRent(): Rent {
		return this.inner.getRent();
	}

	/**
	 * Overwrite the rent sysvar.
	 * @param rent - The new rent object.
	 */
	setRent(rent: Rent) {
		this.inner.setRent(rent);
	}

	/**
	 * Get the SlotHashes sysvar.
	 * @returns The SlotHash array.
	 */
	getSlotHashes(): SlotHash[] {
		return this.inner.getSlotHashes();
	}

	/**
	 * Overwrite the SlotHashes sysvar.
	 * @param hashes - The SlotHash array.
	 */
	setSlotHashes(hashes: SlotHash[]) {
		this.inner.setSlotHashes(hashes);
	}

	/**
	 * Get the SlotHistory sysvar.
	 * @returns The SlotHistory object.
	 */
	getSlotHistory(): SlotHistory {
		return this.inner.getSlotHistory();
	}

	/**
	 * Overwrite the SlotHistory sysvar.
	 * @param history - The SlotHistory object
	 */
	setSlotHistory(history: SlotHistory) {
		this.inner.setSlotHistory(history);
	}

	/**
	 * Get the StakeHistory sysvar.
	 * @returns The StakeHistory object.
	 */
	getStakeHistory(): StakeHistory {
		return this.inner.getStakeHistory();
	}

	/**
	 * Overwrite the StakeHistory sysvar.
	 * @param history - The StakeHistory object
	 */
	setStakeHistory(history: StakeHistory) {
		this.inner.setStakeHistory(history);
	}
}
