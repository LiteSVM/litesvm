import { LiteSVM } from "litesvm";
import { PublicKey, Connection } from "@solana/web3.js";

test("copy accounts from devnet", async () => {
	const owner = PublicKey.unique();
	const usdcMint = new PublicKey(
		"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
	);
	const connection = new Connection("https://api.devnet.solana.com");
	const accountInfo = await connection.getAccountInfo(usdcMint);

	const svm = new LiteSVM();
    svm.setAccount(usdcMint, accountInfo);
	const rawAccount = svm.getAccount(usdcMint);
	expect(rawAccount).not.toBeNull();
});
