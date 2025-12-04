import {
	Address,
	assertIsFullySignedTransaction,
	Blockhash,
	EncodedAccount,
	getAddressCodec,
	getBase58Encoder,
	getTransactionEncoder,
	getTransactionVersionDecoder,
	Lamports,
	lamports,
	MaybeEncodedAccount,
	Signature,
	Transaction,
	TransactionBlockhashLifetime,
} from "@solana/kit";
import {
	Account,
	AddressAndAccount,
	Clock,
	ComputeBudget,
	EpochRewards,
	EpochSchedule,
	FailedTransactionMetadata,
	FeatureSet,
	LiteSvm as LiteSVMInner,
	Rent,
	SimulatedTransactionInfo as SimulatedTransactionInfoInner,
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

function toEncodedAccount(address: Address, account: Account): EncodedAccount {
	const data = account.data();
	return {
		address,
		executable: account.executable(),
		lamports: lamports(account.lamports()),
		programAddress: getAddressCodec().decode(account.owner()),
		space: BigInt(data.length),
		data,
	};
}

export class SimulatedTransactionInfo {
	constructor(inner: SimulatedTransactionInfoInner) {
		this.inner = inner;
	}
	private inner: SimulatedTransactionInfoInner;
	meta(): TransactionMetadata {
		return this.inner.meta();
	}
	postAccounts(): EncodedAccount[] {
		return this.inner
			.postAccounts()
			.map((addressAndAccount: AddressAndAccount) =>
				toEncodedAccount(
					getAddressCodec().decode(addressAndAccount.address),
					addressAndAccount.account(),
				),
			);
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
	withDefaultPrograms(): LiteSVM {
		this.inner.setDefaultPrograms();
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
	getAccount(address: Address): MaybeEncodedAccount {
		const inner = this.inner.getAccount(
			getAddressCodec().encode(address) as Uint8Array,
		);

		return inner === null
			? { exists: false, address }
			: ({
					exists: true,
					...toEncodedAccount(address, inner),
			  } as MaybeEncodedAccount);
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
	setAccount(account: EncodedAccount): LiteSVM {
		this.inner.setAccount(
			getAddressCodec().encode(account.address) as Uint8Array,
			new Account(
				BigInt(account.lamports),
				account.data as Uint8Array,
				getAddressCodec().encode(account.programAddress) as Uint8Array,
				account.executable,
				0n, // rentEpoch was deprecated from the RPC response and removed from Kit.
			),
		);
		return this;
	}

	/**
	 * Gets the balance of the provided account address.
	 * @param address - The account address.
	 * @returns The account's balance in lamports.
	 */
	getBalance(address: Address): Lamports | null {
		const addressBytes = getAddressCodec().encode(address) as Uint8Array;
		const balance = this.inner.getBalance(addressBytes);

		return balance === null ? null : lamports(balance);
	}

	/**
	 * Gets the latest blockhash.
	 * Since LiteSVM doesn't have blocks, this is an arbitrary value controlled by LiteSVM
	 * @returns The designated latest blockhash.
	 */
	latestBlockhash(): Blockhash {
		return this.inner.latestBlockhash() as Blockhash;
	}

	/**
	 * Gets the latest blockhash and last valid block height.
	 * Since LiteSVM doesn't have blocks, this is an arbitrary value controlled by LiteSVM
	 * @returns The designated latest blockhash and last valid block height.
	 */
	latestBlockhashLifetime(): TransactionBlockhashLifetime {
		return {
			blockhash: this.inner.latestBlockhash() as Blockhash,
			lastValidBlockHeight: 0n,
		};
	}

	/**
	 * Gets a transaction from the transaction history.
	 * @param signature - The transaction signature bytes
	 * @returns The transaction, if it is found in the history.
	 */
	getTransaction(
		signature: Signature,
	): TransactionMetadata | FailedTransactionMetadata | null {
		const signatureBytes = getBase58Encoder().encode(signature) as Uint8Array;
		return this.inner.getTransaction(signatureBytes);
	}

	/**
	 * Airdrops the lamport amount specified to the given address.
	 * @param address The airdrop recipient.
	 * @param lamports - The amount to airdrop.
	 * @returns The transaction result.
	 */
	airdrop(
		address: Address,
		lamports: Lamports,
	): TransactionMetadata | FailedTransactionMetadata | null {
		return this.inner.airdrop(
			getAddressCodec().encode(address) as Uint8Array,
			lamports,
		);
	}

	/**
	 * Adds an SBF program to the test environment from the file specified.
	 * @param programId - The program ID.
	 * @param path - The path to the .so file.
	 */
	addProgramFromFile(programId: Address, path: string): LiteSVM {
		this.inner.addProgramFromFile(
			getAddressCodec().encode(programId) as Uint8Array,
			path,
		);
		return this;
	}

	/**
	 * Adds am SBF program to the test environment.
	 * @param programId - The program ID.
	 * @param programBytes - The raw bytes of the compiled program.
	 */
	addProgram(programId: Address, programBytes: Uint8Array): LiteSVM {
		this.inner.addProgram(
			getAddressCodec().encode(programId) as Uint8Array,
			programBytes,
		);
		return this;
	}

	/**
	 * Processes a transaction and returns the result.
	 * @param tx - The transaction to send.
	 * @returns TransactionMetadata if the transaction succeeds, else FailedTransactionMetadata
	 */
	sendTransaction(
		tx: Transaction,
	): TransactionMetadata | FailedTransactionMetadata {
		const internal = this.inner;
		if (internal.getSigverify()) {
			assertIsFullySignedTransaction(tx);
		}

		// The version is located at the beginning of the message bytes.
		const version = getTransactionVersionDecoder().decode(tx.messageBytes);
		const serialized = getTransactionEncoder().encode(tx) as Uint8Array;

		switch (version) {
			case "legacy":
				return internal.sendLegacyTransaction(serialized);
			case 0:
				return internal.sendVersionedTransaction(serialized);
			default:
				throw new Error(`Unsupported transaction version: ${version}`);
		}
	}

	/**
	 * Simulates a transaction
	 * @param tx The transaction to simulate
	 * @returns SimulatedTransactionInfo if simulation succeeds, else FailedTransactionMetadata
	 */
	simulateTransaction(
		tx: Transaction,
	): FailedTransactionMetadata | SimulatedTransactionInfo {
		const internal = this.inner;
		if (internal.getSigverify()) {
			assertIsFullySignedTransaction(tx);
		}

		// The version is located at the beginning of the message bytes.
		const version = getTransactionVersionDecoder().decode(tx.messageBytes);
		const serialized = getTransactionEncoder().encode(tx) as Uint8Array;

		const inner = (() => {
			switch (version) {
				case "legacy":
					return internal.simulateLegacyTransaction(serialized);
				case 0:
					return internal.simulateVersionedTransaction(serialized);
				default:
					throw new Error(`Unsupported transaction version: ${version}`);
			}
		})();

		if (inner instanceof FailedTransactionMetadata) {
			return inner;
		}

		return new SimulatedTransactionInfo(inner);
	}

	/**
	 * Expires the current blockhash.
	 * The return value of `latestBlockhash()` will be different after calling this.
	 */
	expireBlockhash(): LiteSVM {
		this.inner.expireBlockhash();
		return this;
	}

	/**
	 * Warps the clock to the specified slot. This is a convenience wrapper
	 * around `setClock()`.
	 * @param slot - The new slot.
	 */
	warpToSlot(slot: bigint): LiteSVM {
		this.inner.warpToSlot(slot);
		return this;
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
	setClock(clock: Clock): LiteSVM {
		this.inner.setClock(clock);
		return this;
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
	setEpochRewards(rewards: EpochRewards): LiteSVM {
		this.inner.setEpochRewards(rewards);
		return this;
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
	setEpochSchedule(schedule: EpochSchedule): LiteSVM {
		this.inner.setEpochSchedule(schedule);
		return this;
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
	setLastRestartSlot(slot: bigint): LiteSVM {
		this.inner.setLastRestartSlot(slot);
		return this;
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
	setRent(rent: Rent): LiteSVM {
		this.inner.setRent(rent);
		return this;
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
	setSlotHashes(hashes: SlotHash[]): LiteSVM {
		this.inner.setSlotHashes(hashes);
		return this;
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
	setSlotHistory(history: SlotHistory): LiteSVM {
		this.inner.setSlotHistory(history);
		return this;
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
	setStakeHistory(history: StakeHistory): LiteSVM {
		this.inner.setStakeHistory(history);
		return this;
	}

	/**
	 * Helper method to apply a function to the LiteSVM instance in a chain.
	 * @param fn - The function to apply.
	 */
	tap(fn: (svm: LiteSVM) => void): LiteSVM {
		fn(this);
		return this;
	}
}
