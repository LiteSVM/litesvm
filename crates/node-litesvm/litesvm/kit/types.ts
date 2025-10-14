// Kit type definitions for LiteSVM integration
import type {
	Address,
	Signature,
	TransactionError,
} from "@solana/kit";

// Kit-compatible account type
export interface KitAccountInfo {
	readonly address: Address;
	readonly executable: boolean;
	readonly lamports: bigint;
	readonly data: Uint8Array;
	readonly owner: Address;
	readonly rentEpoch: bigint;
}

// Kit-compatible transaction result types
export interface KitTransactionMetadata {
	readonly signature: Signature;
	readonly slot: bigint;
	readonly computeUnitsConsumed: bigint;
	readonly fee: bigint;
	readonly innerInstructions: readonly KitInnerInstruction[];
	readonly logMessages: readonly string[];
	readonly returnData?: {
		readonly programId: Address;
		readonly data: Uint8Array;
	};
	readonly preBalances: readonly bigint[];
	readonly postBalances: readonly bigint[];
	readonly preTokenBalances: readonly any[]; // TODO: Define proper token balance type
	readonly postTokenBalances: readonly any[]; // TODO: Define proper token balance type
}

export interface KitFailedTransactionMetadata {
	readonly signature: Signature;
	readonly slot: bigint;
	readonly computeUnitsConsumed: bigint;
	readonly fee: bigint;
	readonly innerInstructions: readonly KitInnerInstruction[];
	readonly logMessages: readonly string[];
	readonly err: TransactionError;
}

export interface KitInnerInstruction {
	readonly programId: Address;
	readonly accounts: readonly Address[];
	readonly data: Uint8Array;
}

// Utility type for kit-compatible simulation results
export interface KitSimulatedTransactionInfo {
	meta(): KitTransactionMetadata;
	postAccounts(): readonly [Address, KitAccountInfo][];
}

// Type guards
export function isKitFailedTransaction(
	result: KitTransactionMetadata | KitFailedTransactionMetadata,
): result is KitFailedTransactionMetadata {
	return "error" in result;
}

export function isKitSuccessfulTransaction(
	result: KitTransactionMetadata | KitFailedTransactionMetadata,
): result is KitTransactionMetadata {
	return !("error" in result);
}