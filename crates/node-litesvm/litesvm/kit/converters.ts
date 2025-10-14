import {
	Address,
	address,
	Transaction as KitTransaction,
	Signature,
	signature,
} from "@solana/kit";
import bs58 from "bs58";

/**
 * Core utility functions for @solana/kit integration
 * These functions work purely with @solana/kit types and native bindings
 */

// Address/bytes conversion utilities
export function addressToBytes(addr: Address): Uint8Array {
	return bs58.decode(addr);
}

export function addressFromBytes(bytes: Uint8Array): Address {
	return address(bs58.encode(bytes));
}

// Signature/bytes conversion utilities  
export function signatureToBytes(sig: Signature): Uint8Array {
	return bs58.decode(sig);
}

export function signatureFromBytes(bytes: Uint8Array): Signature {
	return signature(bs58.encode(bytes));
}

// Mock transaction serialization for Kit transactions
// TODO: Replace with actual @solana/kit serialization once API is available
export function serializeKitTransaction(tx: KitTransaction): Uint8Array {
	// Placeholder implementation - need actual Kit serialization
	throw new Error("Kit transaction serialization not yet implemented - awaiting @solana/kit API");
}

export function deserializeToKitTransaction(serialized: Uint8Array): KitTransaction {
	// Placeholder implementation - need actual Kit deserialization
	throw new Error("Kit transaction deserialization not yet implemented - awaiting @solana/kit API");
}

// Type guards for Kit types
export function isKitTransaction(tx: any): tx is KitTransaction {
	// Basic type guard - may need refinement based on actual Kit transaction structure
	return tx && typeof tx === 'object' && 'instructions' in tx;
}