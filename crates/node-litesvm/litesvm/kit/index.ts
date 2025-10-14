// Kit integration exports - provides @solana/kit compatible interface

// Re-export Kit types and utilities
export type {
	KitAccountInfo,
	KitTransactionMetadata,
	KitFailedTransactionMetadata,
	KitSimulatedTransactionInfo,
	KitInnerInstruction,
} from "./types";

// Re-export Kit factory interface and implementation
export type { LiteSVMKit } from "./factory";
export { LiteSVMKit as LiteSVMKitClass } from "./extensions";
export { createLiteSVM, createLiteSVMDefault } from "./factory";

// Re-export Kit converters
export {
	addressFromPublicKey,
	publicKeyFromAddress,
	kitAccountFromAccount,
	accountFromKitAccount,
	signatureFromTransactionSignature,
	transactionSignatureFromSignature,
	kitTransactionMetadataFromTransactionMetadata,
	kitFailedTransactionMetadataFromFailedTransactionMetadata,
	isLegacyTransaction,
	isVersionedTransaction,
	isKitTransaction,
} from "./converters";

// Re-export Kit type guards
export {
	isKitFailedTransaction,
	isKitSuccessfulTransaction,
} from "./types";