import {
  Address,
  address,
  Signature,
  signature,
  getCompiledTransactionMessageDecoder,
  signTransactionMessageWithSigners,
  getTransactionEncoder,
} from "@solana/kit";
import bs58 from "bs58";
import type { KitTransactionMessage } from "./types.js";

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

// Kit transaction serialization using proper @solana/kit API
export async function serializeKitTransactionMessage(
  txMessage: KitTransactionMessage
): Promise<Uint8Array> {
  try {
    // Sign the transaction message with all signers embedded in it
    const signedTransaction = await signTransactionMessageWithSigners(
      txMessage as any
    );

    // Get the transaction encoder
    const encoder = getTransactionEncoder();

    // Encode the signed transaction to bytes
    const encoded = encoder.encode(signedTransaction);
    return new Uint8Array(encoded);
  } catch (error) {
    throw new Error(`Failed to serialize Kit transaction message: ${error}`);
  }
}

export function deserializeToKitTransactionMessage(
  serialized: Uint8Array
): any {
  try {
    // Get the decoder for compiled transaction messages
    const decoder = getCompiledTransactionMessageDecoder();

    // Decode from bytes - use type assertion to work around Kit type issues
    const compiledMessage = decoder.decode(serialized as any);

    return compiledMessage;
  } catch (error) {
    throw new Error(`Failed to deserialize Kit transaction message: ${error}`);
  }
}

// Type guards for Kit types
export function isKitTransactionMessage(tx: any): tx is KitTransactionMessage {
  // Basic type guard for transaction message structure
  return (
    tx && typeof tx === "object" && "instructions" in tx && "feePayer" in tx
  );
}
