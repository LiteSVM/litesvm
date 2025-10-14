// Kit-native factory for LiteSVM
import type {
	Address,
	Transaction as KitTransaction,
	Signature,
} from "@solana/kit";
import { LiteSVM } from "../core";
import type {
	KitAccountInfo,
	KitTransactionMetadata,
	KitFailedTransactionMetadata,
	KitSimulatedTransactionInfo,
} from "./types";
import type {
	Clock,
	ComputeBudget,
	EpochRewards,
	EpochSchedule,
	FeatureSet,
	Rent,
	SlotHash,
	SlotHistory,
	StakeHistory,
	FailedTransactionMetadata,
	TransactionMetadata,
} from "../internal";

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
import {
	addressFromPublicKey,
	publicKeyFromAddress,
	kitAccountFromAccount,
	accountFromKitAccount,
	kitTransactionMetadataFromTransactionMetadata,
	kitFailedTransactionMetadataFromFailedTransactionMetadata,
} from "./converters";

/**
 * Kit-native LiteSVM interface
 * 
 * This interface provides a Kit-first API that works exclusively with Kit types,
 * eliminating the need for manual type conversions.
 */
export interface LiteSVMKit {
	// Configuration methods
	withComputeBudget(budget: ComputeBudget): LiteSVMKit;
	withSigverify(sigverify: boolean): LiteSVMKit;
	withBlockhashCheck(check: boolean): LiteSVMKit;
	withSysvars(): LiteSVMKit;
	withFeatureSet(featureSet: FeatureSet): LiteSVMKit;
	withBuiltins(): LiteSVMKit;
	withLamports(lamports: bigint): LiteSVMKit;
	withDefaultPrograms(): LiteSVMKit;
	withTransactionHistory(capacity: bigint): LiteSVMKit;
	withLogBytesLimit(limit?: bigint): LiteSVMKit;
	withPrecompiles(): LiteSVMKit;

	// Core functionality
	minimumBalanceForRentExemption(dataLen: bigint): bigint;
	getAccount(address: Address): KitAccountInfo | null;
	setAccount(address: Address, account: KitAccountInfo): void;
	getBalance(address: Address): bigint | null;
	latestBlockhash(): string;
	getTransaction(signature: Signature): KitTransactionMetadata | KitFailedTransactionMetadata | null;
	airdrop(address: Address, lamports: bigint): KitTransactionMetadata | KitFailedTransactionMetadata | null;
	addProgramFromFile(programId: Address, path: string): void;
	addProgram(programId: Address, programBytes: Uint8Array): void;
	sendTransaction(tx: KitTransaction): KitTransactionMetadata | KitFailedTransactionMetadata;
	simulateTransaction(tx: KitTransaction): KitFailedTransactionMetadata | KitSimulatedTransactionInfo;
	expireBlockhash(): void;
	warpToSlot(slot: bigint): void;

	// Sysvar access
	getClock(): Clock;
	setClock(clock: Clock): void;
	getEpochRewards(): EpochRewards;
	setEpochRewards(rewards: EpochRewards): void;
	getEpochSchedule(): EpochSchedule;
	setEpochSchedule(schedule: EpochSchedule): void;
	getLastRestartSlot(): bigint;
	setLastRestartSlot(slot: bigint): void;
	getRent(): Rent;
	setRent(rent: Rent): void;
	getSlotHashes(): SlotHash[];
	setSlotHashes(hashes: SlotHash[]): void;
	getSlotHistory(): SlotHistory;
	setSlotHistory(history: SlotHistory): void;
	getStakeHistory(): StakeHistory;
	setStakeHistory(history: StakeHistory): void;
}

/**
 * Kit-native LiteSVM implementation
 * 
 * This class wraps the base LiteSVM functionality and provides a Kit-first API.
 */
class LiteSVMKitImpl implements LiteSVMKit {
	private inner: LiteSVM;

	constructor(inner: LiteSVM) {
		this.inner = inner;
	}

	// Configuration methods that return Kit interface
	withComputeBudget(budget: ComputeBudget): LiteSVMKit {
		this.inner.withComputeBudget(budget);
		return this;
	}

	withSigverify(sigverify: boolean): LiteSVMKit {
		this.inner.withSigverify(sigverify);
		return this;
	}

	withBlockhashCheck(check: boolean): LiteSVMKit {
		this.inner.withBlockhashCheck(check);
		return this;
	}

	withSysvars(): LiteSVMKit {
		this.inner.withSysvars();
		return this;
	}

	withFeatureSet(featureSet: FeatureSet): LiteSVMKit {
		this.inner.withFeatureSet(featureSet);
		return this;
	}

	withBuiltins(): LiteSVMKit {
		this.inner.withBuiltins();
		return this;
	}

	withLamports(lamports: bigint): LiteSVMKit {
		this.inner.withLamports(lamports);
		return this;
	}

	withDefaultPrograms(): LiteSVMKit {
		this.inner.withDefaultPrograms();
		return this;
	}

	withTransactionHistory(capacity: bigint): LiteSVMKit {
		this.inner.withTransactionHistory(capacity);
		return this;
	}

	withLogBytesLimit(limit?: bigint): LiteSVMKit {
		this.inner.withLogBytesLimit(limit);
		return this;
	}

	withPrecompiles(): LiteSVMKit {
		this.inner.withPrecompiles();
		return this;
	}

	// Core functionality with Kit types
	minimumBalanceForRentExemption(dataLen: bigint): bigint {
		return this.inner.minimumBalanceForRentExemption(dataLen);
	}

	getAccount(address: Address): KitAccountInfo | null {
		const publicKey = publicKeyFromAddress(address);
		const account = this.inner.getAccount(publicKey);
		if (!account) return null;

		return {
			address,
			executable: account.executable,
			lamports: BigInt(account.lamports),
			data: account.data,
			owner: addressFromPublicKey(account.owner),
			rentEpoch: BigInt(account.rentEpoch || 0),
		};
	}

	setAccount(address: Address, account: KitAccountInfo): void {
		const publicKey = publicKeyFromAddress(address);
		this.inner.setAccount(publicKey, {
			executable: account.executable,
			lamports: Number(account.lamports),
			data: account.data,
			owner: publicKeyFromAddress(account.owner),
			rentEpoch: Number(account.rentEpoch),
		});
	}

	getBalance(address: Address): bigint | null {
		const publicKey = publicKeyFromAddress(address);
		return this.inner.getBalance(publicKey);
	}

	latestBlockhash(): string {
		return this.inner.latestBlockhash();
	}

	getTransaction(signature: Signature): KitTransactionMetadata | KitFailedTransactionMetadata | null {
		// Convert Kit signature to bytes format expected by inner implementation
		const signatureBytes = new TextEncoder().encode(signature);
		const result = this.inner.getTransaction(signatureBytes);
		
		if (!result) return null;
		
		if (isFailedTransactionMetadata(result)) {
			return kitFailedTransactionMetadataFromFailedTransactionMetadata(result);
		} else {
			return kitTransactionMetadataFromTransactionMetadata(result);
		}
	}

	airdrop(address: Address, lamports: bigint): KitTransactionMetadata | KitFailedTransactionMetadata | null {
		const publicKey = publicKeyFromAddress(address);
		const result = this.inner.airdrop(publicKey, lamports);
		
		if (!result) return null;
		
		if (isFailedTransactionMetadata(result)) {
			return kitFailedTransactionMetadataFromFailedTransactionMetadata(result);
		} else {
			return kitTransactionMetadataFromTransactionMetadata(result);
		}
	}

	addProgramFromFile(programId: Address, path: string): void {
		const publicKey = publicKeyFromAddress(programId);
		this.inner.addProgramFromFile(publicKey, path);
	}

	addProgram(programId: Address, programBytes: Uint8Array): void {
		const publicKey = publicKeyFromAddress(programId);
		this.inner.addProgram(publicKey, programBytes);
	}

	sendTransaction(tx: KitTransaction): KitTransactionMetadata | KitFailedTransactionMetadata {
		// TODO: Implement proper Kit transaction handling
		throw new Error("Kit transaction support not yet fully implemented. Please use the compat layer or web3.js transactions for now.");
	}

	simulateTransaction(tx: KitTransaction): KitFailedTransactionMetadata | KitSimulatedTransactionInfo {
		// TODO: Implement proper Kit transaction simulation
		throw new Error("Kit transaction simulation not yet fully implemented. Please use the compat layer or web3.js transactions for now.");
	}

	expireBlockhash(): void {
		this.inner.expireBlockhash();
	}

	warpToSlot(slot: bigint): void {
		this.inner.warpToSlot(slot);
	}

	// Sysvar access (these pass through directly as they don't use addresses)
	getClock(): Clock {
		return this.inner.getClock();
	}

	setClock(clock: Clock): void {
		this.inner.setClock(clock);
	}

	getEpochRewards(): EpochRewards {
		return this.inner.getEpochRewards();
	}

	setEpochRewards(rewards: EpochRewards): void {
		this.inner.setEpochRewards(rewards);
	}

	getEpochSchedule(): EpochSchedule {
		return this.inner.getEpochSchedule();
	}

	setEpochSchedule(schedule: EpochSchedule): void {
		this.inner.setEpochSchedule(schedule);
	}

	getLastRestartSlot(): bigint {
		return this.inner.getLastRestartSlot();
	}

	setLastRestartSlot(slot: bigint): void {
		this.inner.setLastRestartSlot(slot);
	}

	getRent(): Rent {
		return this.inner.getRent();
	}

	setRent(rent: Rent): void {
		this.inner.setRent(rent);
	}

	getSlotHashes(): SlotHash[] {
		return this.inner.getSlotHashes();
	}

	setSlotHashes(hashes: SlotHash[]): void {
		this.inner.setSlotHashes(hashes);
	}

	getSlotHistory(): SlotHistory {
		return this.inner.getSlotHistory();
	}

	setSlotHistory(history: SlotHistory): void {
		this.inner.setSlotHistory(history);
	}

	getStakeHistory(): StakeHistory {
		return this.inner.getStakeHistory();
	}

	setStakeHistory(history: StakeHistory): void {
		this.inner.setStakeHistory(history);
	}
}

/**
 * Create a new Kit-native LiteSVM instance with standard functionality enabled
 * 
 * @returns A LiteSVM instance that works exclusively with Kit types
 */
export function createLiteSVM(): LiteSVMKit {
	const inner = new LiteSVM();
	return new LiteSVMKitImpl(inner);
}

/**
 * Create a new Kit-native LiteSVM instance with minimal functionality enabled
 * 
 * @returns A LiteSVM instance that works exclusively with Kit types
 */
export function createLiteSVMDefault(): LiteSVMKit {
	const inner = LiteSVM.default();
	return new LiteSVMKitImpl(inner);
}