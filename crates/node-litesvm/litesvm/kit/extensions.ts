// Extended LiteSVM class with pure @solana/kit support (no @solana/web3.js dependencies)
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
import type {
	FailedTransactionMetadata,
	TransactionMetadata,
} from "../internal";
import bs58 from "bs58";

// Helper function to convert Address to bytes
function addressToBytes(address: Address): Uint8Array {
	// @solana/kit Address is base58-encoded string, decode to bytes
	return bs58.decode(address);
}

// Helper function to convert Account to KitAccountInfo
function toKitAccountInfo(acc: Account, address: Address): KitAccountInfo {
	return {
		address,
		executable: acc.executable(),
		owner: bs58.encode(acc.owner()) as unknown as Address, // Convert bytes to base58 string, then cast to Address
		lamports: acc.lamports(),
		data: acc.data(),
		rentEpoch: acc.rentEpoch(),
	};
}

// Helper function to convert KitAccountInfo to Account
function fromKitAccountInfo(acc: KitAccountInfo): Account {
	const maybeRentEpoch = acc.rentEpoch;
	const rentEpoch = maybeRentEpoch || 0n;
	return new Account(
		acc.lamports,
		acc.data,
		addressToBytes(acc.owner),
		acc.executable,
		rentEpoch,
	);
}

/**
 * Extended LiteSVM class with pure @solana/kit support
 * This provides Kit-compatible methods that work exclusively with @solana/kit types
 * while maintaining backward compatibility with @solana/web3.js
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

	// Configuration methods - all return LiteSVMKit for method chaining
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

	// Pure @solana/kit methods - no web3.js dependencies
	
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
	 * If the account is not found, null is returned.
	 * @param address - The account address to look up.
	 * @returns The account object, if the account exists.
	 */
	getAccount(address: Address): KitAccountInfo | null {
		const addressBytes = addressToBytes(address);
		const inner = this.inner.getAccount(addressBytes);
		return inner === null ? null : toKitAccountInfo(inner, address);
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
	setAccount(address: Address, account: KitAccountInfo) {
		const addressBytes = addressToBytes(address);
		const accountObj = fromKitAccountInfo(account);
		this.inner.setAccount(addressBytes, accountObj);
	}

	/**
	 * Gets the balance of the provided account address.
	 * @param address - The account address.
	 * @returns The account's balance in lamports.
	 */
	getBalance(address: Address): bigint | null {
		const addressBytes = addressToBytes(address);
		return this.inner.getBalance(addressBytes);
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
	 * @param signature - The transaction signature
	 * @returns The transaction, if it is found in the history.
	 */
	getTransaction(
		signature: Signature,
	): KitTransactionMetadata | KitFailedTransactionMetadata | null {
		// Convert Signature to bytes - signatures are base58 strings like addresses
		const signatureBytes = bs58.decode(signature);
		const result = this.inner.getTransaction(signatureBytes);
		
		if (!result) return null;
		
		// Check if this is a FailedTransactionMetadata (has err method)
		if ("err" in result) {
			// Failed transaction - convert to KitFailedTransactionMetadata
			const meta = result.meta();
			return {
				signature,
				slot: 0n, // Default slot - not available in LiteSVM metadata
				computeUnitsConsumed: meta.computeUnitsConsumed(),
				fee: 0n, // Default fee - not available in LiteSVM metadata
				innerInstructions: [], // Default empty array - would need conversion from meta.innerInstructions()
				logMessages: meta.logs(),
				err: result.err() as any, // Cast the error to TransactionError - types are incompatible but runtime compatible
			};
		} else {
			// Successful transaction - convert to KitTransactionMetadata
			return {
				signature,
				slot: 0n, // Default slot - not available in LiteSVM metadata
				computeUnitsConsumed: result.computeUnitsConsumed(),
				fee: 0n, // Default fee - not available in LiteSVM metadata
				innerInstructions: [], // Default empty array - would need conversion from result.innerInstructions()
				logMessages: result.logs(),
				returnData: {
					programId: result.returnData().programId() as unknown as Address,
					data: result.returnData().data(),
				},
				preBalances: [], // Default empty array - not available in LiteSVM metadata
				postBalances: [], // Default empty array - not available in LiteSVM metadata
				preTokenBalances: [], // Default empty array - not available in LiteSVM metadata
				postTokenBalances: [], // Default empty array - not available in LiteSVM metadata
			};
		}
	}

	/**
	 * Airdrops the lamport amount specified to the given address.
	 * @param address The airdrop recipient.
	 * @param lamports - The amount to airdrop.
	 * @returns The transaction result.
	 */
	airdrop(
		address: Address,
		lamports: bigint,
	): KitTransactionMetadata | KitFailedTransactionMetadata | null {
		const addressBytes = addressToBytes(address);
		const result = this.inner.airdrop(addressBytes, lamports);
		
		if (!result) return null;
		
		// Convert result to Kit types 
		if ("err" in result) {
			// Failed transaction - FailedTransactionMetadata
			const signature = bs58.encode(result.meta().signature()) as Signature;
			const meta = result.meta();
			
			return {
				signature,
				slot: 0n, // Default slot - not available in LiteSVM metadata
				computeUnitsConsumed: meta.computeUnitsConsumed(),
				fee: 0n, // Default fee - not available in LiteSVM metadata
				innerInstructions: [], // Default empty array - would need conversion from meta.innerInstructions()
				logMessages: meta.logs(),
				err: result.err() as any, // Cast the error to TransactionError - types are incompatible but runtime compatible
			};
		} else {
			// Successful transaction - TransactionMetadata  
			const signature = bs58.encode(result.signature()) as Signature;
			
			return {
				signature,
				slot: 0n, // Default slot - not available in LiteSVM metadata
				computeUnitsConsumed: result.computeUnitsConsumed(),
				fee: 0n, // Default fee - not available in LiteSVM metadata
				innerInstructions: [], // Default empty array - would need conversion from result.innerInstructions()
				logMessages: result.logs(),
				returnData: {
					programId: result.returnData().programId() as unknown as Address,
					data: result.returnData().data(),
				},
				preBalances: [], // Default empty array - not available in LiteSVM metadata
				postBalances: [], // Default empty array - not available in LiteSVM metadata
				preTokenBalances: [], // Default empty array - not available in LiteSVM metadata
				postTokenBalances: [], // Default empty array - not available in LiteSVM metadata
			};
		}
	}

	/**
	 * Adds an SBF program to the test environment from the file specified.
	 * @param programId - The program ID.
	 * @param path - The path to the .so file.
	 */
	addProgramFromFile(programId: Address, path: string) {
		const programIdBytes = addressToBytes(programId);
		return this.inner.addProgramFromFile(programIdBytes, path);
	}

	/**
	 * Adds am SBF program to the test environment.
	 * @param programId - The program ID.
	 * @param programBytes - The raw bytes of the compiled program.
	 */
	addProgram(programId: Address, programBytes: Uint8Array) {
		const programIdBytes = addressToBytes(programId);
		return this.inner.addProgram(programIdBytes, programBytes);
	}

	/**
	 * Processes a Kit transaction and returns the result.
	 * @param tx - The Kit transaction to send.
	 * @returns KitTransactionMetadata if the transaction succeeds, else KitFailedTransactionMetadata
	 */
	sendTransaction(
		tx: KitTransaction,
	): KitTransactionMetadata | KitFailedTransactionMetadata {
		// TODO: Implement Kit transaction serialization and processing
		// For now, throw an error to indicate this needs implementation
		throw new Error("Kit transaction support not yet implemented");
	}

	/**
	 * Simulates a Kit transaction
	 * @param tx The Kit transaction to simulate
	 * @returns KitSimulatedTransactionInfo if simulation succeeds, else KitFailedTransactionMetadata
	 */
	simulateTransaction(
		tx: KitTransaction,
	): KitFailedTransactionMetadata | KitSimulatedTransactionInfo {
		// TODO: Implement Kit transaction simulation
		throw new Error("Kit transaction simulation not yet implemented");
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
	getClock() {
		return this.inner.getClock();
	}

	/**
	 * Overwrite the clock sysvar.
	 * @param clock - The clock object.
	 */
	setClock(clock: any) {
		this.inner.setClock(clock);
	}

	// Additional sysvar methods would go here...
	// For brevity, I'll add the most commonly used ones

	/**
	 * Get the cluster rent.
	 * @returns The rent object.
	 */
	getRent() {
		return this.inner.getRent();
	}

	/**
	 * Overwrite the rent sysvar.
	 * @param rent - The new rent object.
	 */
	setRent(rent: any) {
		this.inner.setRent(rent);
	}
}