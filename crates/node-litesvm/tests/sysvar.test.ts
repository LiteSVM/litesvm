import { LiteSVM, Rent, Clock } from "litesvm";

test("sysvar", () => {
	const svm = new LiteSVM();
	const rentBefore = svm.getRent();
	expect(rentBefore.burnPercent).toBe(50);
	expect(rentBefore.minimumBalance(123n)).toBe(1746960n);
	const newRent = new Rent(
		rentBefore.lamportsPerByteYear,
		rentBefore.exemptionThreshold,
		0,
	);
	svm.setRent(newRent);
	const rentAfter = svm.getRent();
	expect(rentAfter.burnPercent).toBe(0);
	const clockBefore = svm.getClock();
	expect(clockBefore.epoch).toBe(0n);
	const newClock = new Clock(1000n, 1, 100n, 3n, 4);
	svm.setClock(newClock);
	const clockAfter = svm.getClock();
	expect(clockAfter.epoch).toBe(newClock.epoch);
});
