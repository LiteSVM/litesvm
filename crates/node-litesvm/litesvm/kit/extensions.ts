// Extended LiteSVM class with @solana/kit support
import type {
	Address,
	Transaction as KitTransaction,
	Signature,
} from "@solana/kit";
import { Account, LiteSvm as LiteSVMInner, ComputeBudget, FeatureSet } from "../internal";
import type {
	KitAccountInfo,
	KitTransactionMetadata,
	KitFailedTransactionMetadata,
	KitSimulatedTransactionInfo,
} from "./types";
import {
	addressFromPublicKey,
	publicKeyFromAddress,
	kitAccountFromAccount,
	accountFromKitAccount,
	kitTransactionMetadataFromTransactionMetadata,
	kitFailedTransactionMetadataFromFailedTransactionMetadata,
	serializeKitTransaction,
	isKitTransaction,
} from "./converters";
import {
	PublicKey,
	Transaction,
	VersionedTransaction,
	TransactionSignature,
	AccountInfo,
} from "@solana/web3.js";
import type {
	FailedTransactionMetadata,
	TransactionMetadata,
} from "../internal";

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

// Type guards for transaction metadata
function isFailedTransactionMetadata(
	result: TransactionMetadata | FailedTransactionMetadata
): result is FailedTransactionMetadata {
	return 'err' in result && 'meta' in result;
}

function isSuccessfulTransactionMetadata(
	result: TransactionMetadata | FailedTransactionMetadata
): result is TransactionMetadata {
	return !isFailedTransactionMetadata(result);
}

/**
 * Extended LiteSVM class with @solana/kit support
 * 
 * This class extends the base LiteSVM functionality to work with @solana/kit types
 * while maintaining backward compatibility with @solana/web3.js
 */
/**
 * Extended LiteSVM class with @solana/kit support
 * This provides the same functionality as LiteSVM but with Kit-compatible methods
 */
export class LiteSVMKit {
	private inner: LiteSVMInner;

	/** Create a new LiteSVM instance with standard functionality enabled */
	constructor() {
		this.inner = new LiteSVMInner();
	}

	/** Create a new LiteSVM instance with minimal functionality enabled */
	static default(): LiteSVMKit {
		const svm = new LiteSVMKit();
		const inner = LiteSVMInner.default();
		svm.inner = inner;
		return svm;
	}

	// Delegate all LiteSVM methods to the inner instance
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

	// Standard LiteSVM methods
	minimumBalanceForRentExemption(dataLen: bigint): bigint {
		return this.inner.minimumBalanceForRentExemption(dataLen);
	}

	getAccount(address: PublicKey): AccountInfoBytes | null {
		const inner = this.inner.getAccount(address.toBytes());
		return inner === null ? null : toAccountInfo(inner);
	}

	setAccount(address: PublicKey, account: AccountInfoBytes) {
		this.inner.setAccount(address.toBytes(), fromAccountInfo(account));
	}

	getBalance(address: PublicKey): bigint | null {
		return this.inner.getBalance(address.toBytes());
	}

	latestBlockhash(): string {
		return this.inner.latestBlockhash();
	}

	// Transaction methods would go here...
	// For now, let's focus on the Kit methods
	/**
	 * Return the account at the given address (Kit version).
	 * If the account is not found, null is returned.
	 * @param address - The account address to look up.
	 * @returns The account object, if the account exists.
	 */
	getAccountKit(address: Address): KitAccountInfo | null {
		const publicKey = publicKeyFromAddress(address);
		const accountInfo = this.getAccount(publicKey);
		if (!accountInfo) return null;
		
		// Convert AccountInfoBytes to Account first
		const maybeRentEpoch = accountInfo.rentEpoch;
		const rentEpoch = maybeRentEpoch || 0;
		const account = new Account(
			BigInt(accountInfo.lamports),
			accountInfo.data,
			accountInfo.owner.toBytes(),
			accountInfo.executable,
			BigInt(rentEpoch),
		);
		
		return kitAccountFromAccount(account, address);
	}

	/**
	 * Create or overwrite an account, subverting normal runtime checks (Kit version).
	 *
	 * This method exists to make it easier to set up artificial situations
	 * that would be difficult to replicate by sending individual transactions.
	 * Beware that it can be used to create states that would not be reachable
	 * by sending transactions!
	 *
	 * @param address - The address to write to.
	 * @param account - The account object to write.
	 */
	setAccountKit(address: Address, account: KitAccountInfo) {
		const publicKey = publicKeyFromAddress(address);
		const web3Account = accountFromKitAccount(account);
		this.setAccount(publicKey, {
			executable: account.executable,
			owner: publicKeyFromAddress(account.owner),
			lamports: Number(account.lamports),
			data: account.data,
			rentEpoch: Number(account.rentEpoch),
		});
	}

	/**
	 * Gets the balance of the provided account address (Kit version).
	 * @param address - The account address.
	 * @returns The account's balance in lamports.
	 */
	getBalanceKit(address: Address): bigint | null {
		const publicKey = publicKeyFromAddress(address);
		return this.getBalance(publicKey);
	}
}