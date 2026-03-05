import { getAddressEncoder } from "@solana/kit";
import assert from "node:assert";
import { describe, it } from "node:test";
import { FeatureSet } from "../litesvm";
import { generateAddress } from "./util";

describe("FeatureSet", () => {
  it("should create default feature set", () => {
    const fs = new FeatureSet();
    assert.ok(fs);
  });

  it("should create all_enabled feature set", () => {
    const fs = FeatureSet.allEnabled();
    assert.ok(fs);
  });

  it("should activate and check feature", async () => {
    const fs = new FeatureSet();
    const featureId = getAddressEncoder().encode(await generateAddress()) as Uint8Array;

    const isActiveBeforeActivation = fs.isActive(featureId);
    assert.strictEqual(isActiveBeforeActivation, false);

    fs.activate(featureId, 100n);

    const isActiveAfterActivation = fs.isActive(featureId);
    assert.strictEqual(isActiveAfterActivation, true);

    const slot = fs.activatedSlot(featureId);
    assert.strictEqual(slot, 100n);
  });

  it("should deactivate feature", async () => {
    const fs = FeatureSet.allEnabled();
    const featureId = getAddressEncoder().encode(await generateAddress()) as Uint8Array;

    fs.activate(featureId, 50n);
    assert.strictEqual(fs.isActive(featureId), true);

    fs.deactivate(featureId);
    assert.strictEqual(fs.isActive(featureId), false);

    const slot = fs.activatedSlot(featureId);
    assert.strictEqual(slot, null);
  });
});
