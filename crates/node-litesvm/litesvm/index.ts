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

/**
 * The main class in the litesvm library.
 *
 * Use this to send transactions, query accounts and configure the runtime.
 */
export class LiteSVM {
	/** Create a new LiteSVM instance with standard functionality enabled */
	constructor() {
		const inner = new LiteSVMInner();
		this.inner = inner;
	}
	private inner: LiteSVMInner;

	/** Create a new LiteSVM instance with minimal functionality enabled */
	static default(): LiteSVM {
		const svm = new LiteSVM();
		const inner = LiteSVMInner.default();
		svm.inner = inner;
		return svm;
	}

	/**
	 * Set the compute budget
	 * @param budget - The new compute budget
	 * @returns The modified LiteSVM instance
	 */
	withComputeBudget(budget: ComputeBudget): LiteSVM {
		this.inner.setComputeBudget(budget);
		return this;
	}

	/**
	 * Enable or disable sigverify
	 * @param sigverify - if false, transaction signatures will not be checked.
	 * @returns The modified LiteSVM instance
	 */
	withSigverify(sigverify: boolean): LiteSVM {
		this.inner.setSigverify(sigverify);
		return this;
	}

	/**
	 * Enables or disables transaction blockhash checking.
	 * @param check - If false, the blockhash check will be skipped
	 * @returns The modified LiteSVM instance
	 */
	withBlockhashCheck(check: boolean): LiteSVM {
		this.inner.setBlockhashCheck(check);
		return this;
	}

	/**
	 * Sets up the standard sysvars.
	 * @returns The modified LiteSVM instance
	 */
	withSysvars(): LiteSVM {
		this.inner.setSysvars();
		return this;
	}

	/**
	 * Set the FeatureSet used by the VM instance.
	 * @param featureSet The FeatureSet to use.
	 * @returns The modified LiteSVM instance
	 */
	withFeatureSet(featureSet: FeatureSet): LiteSVM {
		this.inner.setFeatureSet(featureSet);
		return this;
	}

	/**
	 * Adds the standard builtin programs. Use `withFeatureSet` beforehand to change change what builtins are added.
	 * @returns The modified LiteSVM instance
	 */
	withBuiltins(): LiteSVM {
		this.inner.setBuiltins();
		return this;
	}

	/**
	 * Changes the initial lamports in LiteSVM's airdrop account.
	 * @param lamports - The number of lamports to set in the airdrop account
	 * @returns The modified LiteSVM instance
	 */
	withLamports(lamports: bigint): LiteSVM {
		this.inner.setLamports(lamports);
		return this;
	}

	/**
	 * Adds the standard SPL programs.
	 * @returns The modified LiteSVM instance
	 */
	withSplPrograms(): LiteSVM {
		this.inner.setSplPrograms();
		return this;
	}

	/**
	 * Changes the capacity of the transaction history.
	 * @param capacity - How many transactions to store in history.
	 * Set this to 0 to disable transaction history and allow duplicate transactions.
	 * @returns The modified LiteSVM instance
	 */
	withTransactionHistory(capacity: bigint): LiteSVM {
		this.inner.setTransactionHistory(capacity);
		return this;
	}

	/**
	 * Set a limit for transaction logs, beyond which they will be truncated.
	 * @param limit - The limit in bytes. If null, no limit is enforced.
	 * @returns The modified LiteSVM instance
	 */
	withLogBytesLimit(limit?: bigint): LiteSVM {
		this.inner.setLogBytesLimit(limit);
		return this;
	}

	/**
	 * Adds the standard precompiles. Use `withFeatureSet` beforehand to change change what builtins are added.
	 * @returns The modified LiteSVM instance
	 */
	withPrecompiles(): LiteSVM {
		this.inner.setPrecompiles();
		return this;
	}

	/**
	 * Calculates the minimum balance required to make an account with specified data length rent exempt.
	 * @param dataLen - The number of bytes in the account.
	 * @returns The required balance in lamports
	 */
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

	/**
	 * Gets the balance of the provided account address.
	 * @param address - The account address.
	 * @returns The account's balance in lamports.
	 */
	getBalance(address: PublicKey): bigint | null {
		return this.inner.getBalance(address.toBytes());
	}

	/**
	 * Gets the latest blockhash.
	 * Since LiteSVM doesn't have blocks, this is an arbitrary value controlled by LiteSVM
	 * @returns The designated latest blockhash.
	 */
	latestBlockhash(): string {
		return this.inner.latestBlockhash();
	}

	/**
	 * Gets a transaction from the transaction history.
	 * @param signature - The transaction signature bytes
	 * @returns The transaction, if it is found in the history.
	 */
	getTransaction(
		signature: Uint8Array,
	): TransactionMetadata | FailedTransactionMetadata | null {
		return this.inner.getTransaction(signature);
	}

	/**
	 * Airdrops the lamport amount specified to the given address.
	 * @param address The airdrop recipient.
	 * @param lamports - The amount to airdrop.
	 * @returns The transaction result.
	 */
	airdrop(
		address: PublicKey,
		lamports: bigint,
	): TransactionMetadata | FailedTransactionMetadata | null {
		return this.inner.airdrop(address.toBytes(), lamports);
	}

	/**
	 * Adds an SBF program to the test environment from the file specified.
	 * @param programId - The program ID.
	 * @param path - The path to the .so file.
	 */
	addProgramFromFile(programId: PublicKey, path: string) {
		return this.inner.addProgramFromFile(programId.toBytes(), path);
	}

	/**
	 * Adds am SBF program to the test environment.
	 * @param programId - The program ID.
	 * @param programBytes - The raw bytes of the compiled program.
	 */
	addProgram(programId: PublicKey, programBytes: Uint8Array) {
		return this.inner.addProgram(programId.toBytes(), programBytes);
	}

	/**
	 * Processes a transaction and returns the result.
	 * @param tx - The transaction to send.
	 * @returns TransactionMetadata if the transaction succeeds, else FailedTransactionMetadata
	 */
	sendTransaction(
		tx: Transaction | VersionedTransaction,
	): TransactionMetadata | FailedTransactionMetadata {
		const internal = this.inner;
		const serialized = tx.serialize({
			requireAllSignatures: true,
			verifySignatures: internal.getSigverify(),
		});

		if (tx instanceof Transaction) {
			return internal.sendLegacyTransaction(serialized);
		} else {
			return internal.sendVersionedTransaction(serialized);
		}
	}

	/**
	 * Simulates a transaction
	 * @param tx The transaction to simulate
	 * @returns SimulatedTransactionInfo if simulation succeeds, else FailedTransactionMetadata
	 */
	simulateTransaction(
		tx: Transaction | VersionedTransaction,
	): FailedTransactionMetadata | SimulatedTransactionInfo {
		const internal = this.inner;
		const serialized = tx.serialize({
			requireAllSignatures: true,
			verifySignatures: internal.getSigverify(),
		});
		const inner =
			tx instanceof Transaction
				? internal.simulateLegacyTransaction(serialized)
				: internal.simulateVersionedTransaction(serialized);
		return inner instanceof FailedTransactionMetadata
			? inner
			: new SimulatedTransactionInfo(inner);
	}

	/**
	 * Expires the current blockhash.
	 * The return value of `latestBlockhash()` will be different after calling this.
	 */
	expireBlockhash() {
		this.inner.expireBlockhash();
	}

	/**
	 * Warps the clock to the specified slot. This is a convenience wrapper
	 * around `setClock()`.
	 * @param slot - The new slot.
	 */
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

	/**
	 * Makes a snapshot of the LiteSVM state
	 */
	snapshot() {
		this.inner.snapshot();
	}

	/**
	 * Reverts LiteSVM state to previously made snapshot.
	 * If there is no snapshot does nothing.
	 */
	revert() {
		this.inner.revert();
	}
}
