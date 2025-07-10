use napi::bindgen_prelude::*;

#[derive(Debug, Clone)]
#[napi]
pub struct SlotHash {
    pub(crate) slot: BigInt,
    pub(crate) hash: String,
}

#[napi]
impl SlotHash {
    #[napi(getter)]
    pub fn slot(&self) -> BigInt {
        self.slot.clone()
    }

    #[napi(getter)]
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    #[napi(setter)]
    pub fn set_slot(&mut self, slot: BigInt) {
        self.slot = slot;
    }

    #[napi(setter)]
    pub fn set_hash(&mut self, hash: String) {
        self.hash = hash;
    }
}
