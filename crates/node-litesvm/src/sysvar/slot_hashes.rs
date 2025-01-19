use napi::bindgen_prelude::*;

#[napi]
pub struct SlotHash {
    pub slot: BigInt,
    pub hash: String,
}
