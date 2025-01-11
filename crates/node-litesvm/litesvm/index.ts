import {
	Account,
	LiteSvm as LiteSVMInner,
	TransactionReturnData as TransactionReturnDataInner,
} from "./internal";
import {
	AccountInfo,
	Keypair,
	PublicKey,
	Transaction,
	Blockhash,
	TransactionSignature,
	Message,
	Commitment,
	VersionedTransaction,
	Cluster,
} from "@solana/web3.js";

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

export class LiteSVM {
	constructor() {
		const inner = new LiteSVMInner();
		this.inner = inner;
	}
	private inner: LiteSVMInner;

	/**
	 * Return the account at the given address.
	 * If the account is not found, None is returned.
	 * @param address - The account address to look up.
	 * @returns The account object, if the account exists.
	 */
	getAccount(address: PublicKey): AccountInfoBytes | null {
		const inner = this.inner.getAccount(address.toBytes());
		return inner === null ? null : toAccountInfo(inner);
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
	setAccount(address: PublicKey, account: AccountInfoBytes) {
		this.inner.setAccount(address.toBytes(), fromAccountInfo(account));
	}
}
