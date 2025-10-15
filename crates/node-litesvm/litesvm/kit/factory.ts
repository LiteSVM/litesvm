import { LiteSVMKit } from "./extensions";

/**
 * Create a new Kit-native LiteSVM instance with standard functionality enabled
 *
 * @returns A LiteSVM instance that works exclusively with Kit types
 */
export function createLiteSVM(): LiteSVMKit {
  return new LiteSVMKit();
}

/**
 * Create a new Kit-native LiteSVM instance with minimal functionality enabled
 *
 * @returns A LiteSVM instance that works exclusively with Kit types
 */
export function createLiteSVMDefault(): LiteSVMKit {
  return LiteSVMKit.default();
}
