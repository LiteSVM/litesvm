// Conversion utilities between @solana/web3.js and @solana/kit types
import {
	Address,
	address,
	Transaction as KitTransaction,
	Signature,
	signature,
} from "@solana/kit";
import {
	PublicKey,
	Transaction,
	VersionedTransaction,
	TransactionSignature,
} from "@solana/web3.js";
import bs58 from "bs58";
import type {
	TransactionMetadata,
	FailedTransactionMetadata,
	InnerInstruction,
} from "../internal";
import {
	Account,
} from "../internal";
import type {
	KitAccountInfo,
	KitTransactionMetadata,
	KitFailedTransactionMetadata,
	KitInnerInstruction,
} from "./types";

// Address/PublicKey conversions
export function addressFromPublicKey(publicKey: PublicKey): Address {
	return address(publicKey.toBase58());
}

export function publicKeyFromAddress(addr: Address): PublicKey {
	return new PublicKey(addr);
}

// Account conversions
export function kitAccountFromAccount(account: Account, addr: Address): KitAccountInfo {
	const ownerPubkey = new PublicKey(account.owner());
	return {
		address: addr,
		executable: account.executable(),
		lamports: account.lamports(),
		data: account.data(),
		owner: address(ownerPubkey.toBase58()),
		rentEpoch: account.rentEpoch(),
	};
}

export function accountFromKitAccount(kitAccount: KitAccountInfo): Account {
	const ownerPubkey = new PublicKey(kitAccount.owner);
	return new Account(
		kitAccount.lamports,
		kitAccount.data,
		ownerPubkey.toBytes(),
		kitAccount.executable,
		kitAccount.rentEpoch,
	);
}

// Transaction signature conversions
export function signatureFromTransactionSignature(txSig: TransactionSignature): Signature {
	return signature(txSig);
}

export function transactionSignatureFromSignature(sig: Signature): TransactionSignature {
	return sig as TransactionSignature;
}

// Transaction metadata conversions
export function kitTransactionMetadataFromTransactionMetadata(
	metadata: TransactionMetadata,
): KitTransactionMetadata {
	const sigBytes = metadata.signature();
	// Convert signature bytes to base58 string
	const sigString = bs58.encode(sigBytes);
	return {
		signature: signature(sigString),
		slot: 0n, // TransactionMetadata doesn't have slot() method, using 0 as placeholder
		logs: metadata.logs(),
		unitsConsumed: metadata.computeUnitsConsumed(),
		returnData: metadata.returnData() ? {
			programId: address(new PublicKey(metadata.returnData().programId()).toBase58()),
			data: metadata.returnData().data(),
		} : undefined,
		innerInstructions: metadata.innerInstructions().flat().map(convertInnerInstruction),
	};
}

export function kitFailedTransactionMetadataFromFailedTransactionMetadata(
	metadata: FailedTransactionMetadata,
): KitFailedTransactionMetadata {
	const meta = metadata.meta();
	const sigBytes = meta.signature();
	const sigPubkey = new PublicKey(sigBytes);
	return {
		signature: signature(sigPubkey.toBase58()),
		error: metadata.err() as any, // Kit's TransactionError should be compatible
		logs: meta.logs(),
		unitsConsumed: meta.computeUnitsConsumed(),
	};
}

function convertInnerInstruction(innerIx: InnerInstruction): KitInnerInstruction {
	const instruction = innerIx.instruction();
	return {
		programId: address("11111111111111111111111111111111"), // Placeholder - need to resolve program ID from index
		accounts: [], // Placeholder - need to resolve account addresses from indices
		data: instruction.data(),
	};
}

// Transaction serialization helpers
export function serializeKitTransaction(tx: KitTransaction): Uint8Array {
	// Kit transactions should have a serialize method
	// This is a placeholder - need to check actual Kit API
	throw new Error("Kit transaction serialization not yet implemented - awaiting Kit API details");
}

export function deserializeToKitTransaction(serialized: Uint8Array): KitTransaction {
	// Kit should provide deserialization utilities
	// This is a placeholder - need to check actual Kit API
	throw new Error("Kit transaction deserialization not yet implemented - awaiting Kit API details");
}

// Type guards and utilities
export function isLegacyTransaction(tx: Transaction | VersionedTransaction | KitTransaction): tx is Transaction {
	return tx instanceof Transaction;
}

export function isVersionedTransaction(tx: Transaction | VersionedTransaction | KitTransaction): tx is VersionedTransaction {
	return tx instanceof VersionedTransaction;
}

export function isKitTransaction(tx: Transaction | VersionedTransaction | KitTransaction): tx is KitTransaction {
	return !isLegacyTransaction(tx) && !isVersionedTransaction(tx);
}