// Kit type definitions for LiteSVM integration
import type {
	Address,
	Signature,
	TransactionError,
	TransactionMessageWithFeePayer,
} from "@solana/kit";

// Kit-compatible transaction message (includes feePayer)
export type KitTransactionMessage = TransactionMessageWithFeePayer<Address>;

// Kit-compatible account type
export interface KitAccountInfo {
	readonly address: Address;
	readonly executable: boolean;
	readonly lamports: bigint;
	readonly data: Uint8Array;
	readonly owner: Address;
	readonly rentEpoch: bigint;
}

// Token balance types for transaction metadata
export interface KitTokenBalance {
	readonly accountIndex: number;
	readonly mint: Address;
	readonly uiTokenAmount: {
		readonly amount: string;
		readonly decimals: number;
		readonly uiAmount?: number;
		readonly uiAmountString: string;
	};
	readonly owner?: Address;
	readonly programId?: Address;
}

// Kit-compatible transaction result types
export interface KitTransactionMetadata {
	readonly signature: Signature;
	readonly slot: bigint;
	readonly computeUnitsConsumed: bigint;
	readonly fee: bigint;
	readonly innerInstructions: readonly KitInnerInstruction[];
	readonly logMessages: readonly string[];
	readonly logs?: readonly string[]; // Add the 'logs' property to the KitTransactionMetadata interface
	readonly returnData?: {
		readonly programId: Address;
		readonly data: Uint8Array;
	};
	readonly preBalances: readonly bigint[];
	readonly postBalances: readonly bigint[];
	readonly preTokenBalances: readonly KitTokenBalance[];
	readonly postTokenBalances: readonly KitTokenBalance[];
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
	return "err" in result;
}

export function isKitSuccessfulTransaction(
	result: KitTransactionMetadata | KitFailedTransactionMetadata,
): result is KitTransactionMetadata {
	return !("err" in result);
}