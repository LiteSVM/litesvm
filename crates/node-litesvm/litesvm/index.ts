import {
	Account,
	LiteSvm as LiteSVMInner,
	TransactionMetadata,
	FailedTransactionMetadata,
	SimulatedTransactionInfo,
} from "./internal";
export {
	TransactionMetadata,
	FailedTransactionMetadata,
	SimulatedTransactionInfo,
	TransactionReturnData,
	InnerInstruction,
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

export class LiteSVM {
	constructor() {
		const inner = new LiteSVMInner();
		this.inner = inner;
	}
	private inner: LiteSVMInner;

	// withComputeBudget

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

	// withBuiltins

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

	// withPrecompiles

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
		if (tx instanceof Transaction) {
			return internal.simulateLegacyTransaction(serialized);
		} else {
			return internal.simulateVersionedTransaction(serialized);
		}
	}

	expireBlockhash() {
		this.inner.expireBlockhash();
	}

	warpToSlot(slot: bigint) {
		this.inner.warpToSlot(slot);
	}
}
