import { LiteSVM } from "litesvm";

test("warp", () => {
	const svm = new LiteSVM();
	const slot0 = svm.getClock().slot;
	expect(slot0).toBe(0n);
	const newSlot = 1000n;
	svm.warpToSlot(newSlot);
	const slot1 = svm.getClock().slot;
	expect(slot1).toBe(newSlot);
});
