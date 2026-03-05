import { describe, it } from "node:test"
import assert from "node:assert"
import { FeatureSet } from "../litesvm"
import { PublicKey } from "@solana/web3.js"

describe("FeatureSet", () => {
    it("should create default feature set", () => {
        const fs = new FeatureSet()
        assert.ok(fs)
    })

    it("should create all_enabled feature set", () => {
        const fs = FeatureSet.allEnabled()
        assert.ok(fs)
    })

    it("should activate and check feature", () => {
        const fs = new FeatureSet()
        const featureId = PublicKey.unique()
        
        const isActiveBeforeActivation = fs.isActive(featureId.toBuffer())
        assert.strictEqual(isActiveBeforeActivation, false)
        
        fs.activate(featureId.toBuffer(), 100n)
        
        const isActiveAfterActivation = fs.isActive(featureId.toBuffer())
        assert.strictEqual(isActiveAfterActivation, true)
        
        const slot = fs.activatedSlot(featureId.toBuffer())
        assert.strictEqual(slot, 100n)
    })

    it("should deactivate feature", () => {
        const fs = FeatureSet.allEnabled()
        const featureId = PublicKey.unique()
        
        fs.activate(featureId.toBuffer(), 50n)
        assert.strictEqual(fs.isActive(featureId.toBuffer()), true)
        
        fs.deactivate(featureId.toBuffer())
        assert.strictEqual(fs.isActive(featureId.toBuffer()), false)
        
        const slot = fs.activatedSlot(featureId.toBuffer())
        assert.strictEqual(slot, null)
    })
})
