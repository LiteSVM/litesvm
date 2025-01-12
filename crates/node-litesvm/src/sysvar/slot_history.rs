use {
    crate::to_string_js,
    bv::BitVec,
    napi::bindgen_prelude::*,
    solana_sdk::slot_history::{Check, SlotHistory as SlotHistoryOriginal},
};

#[derive(Debug)]
#[napi]
pub enum SlotHistoryCheck {
    Future,
    TooOld,
    Found,
    NotFound,
}

to_string_js!(SlotHistoryCheck);

impl From<Check> for SlotHistoryCheck {
    fn from(value: Check) -> Self {
        match value {
            Check::Future => Self::Future,
            Check::TooOld => Self::TooOld,
            Check::Found => Self::Found,
            Check::NotFound => Self::NotFound,
        }
    }
}

/// A bitvector indicating which slots are present in the past epoch.
#[derive(Debug)]
#[napi]
pub struct SlotHistory(pub(crate) SlotHistoryOriginal);

#[napi]
impl SlotHistory {
    #[napi(constructor)]
    pub fn new(bits: BigUint64Array, next_slot: BigInt) -> Self {
        let bits_converted: BitVec<u64> = BitVec::from(bits.to_vec());
        Self(SlotHistoryOriginal {
            bits: bits_converted,
            next_slot: next_slot.get_u64().1,
        })
    }

    #[napi(factory, js_name = "default")]
    pub fn new_default() -> Self {
        Self(SlotHistoryOriginal::default())
    }

    #[napi(getter)]
    pub fn bits(&self) -> BigUint64Array {
        self.0.bits.clone().into_boxed_slice().to_vec().into()
    }

    #[napi(setter)]
    pub fn set_bits(&mut self, bits: BigUint64Array) {
        let bits_converted: BitVec<u64> = BitVec::from(bits.to_vec());
        self.0.bits = bits_converted;
    }

    #[napi(getter)]
    pub fn next_slot(&self) -> u64 {
        self.0.next_slot
    }

    #[napi(setter)]
    pub fn set_next_slot(&mut self, slot: BigInt) {
        self.0.next_slot = slot.get_u64().1;
    }

    #[napi]
    pub fn add(&mut self, slot: BigInt) {
        self.0.add(slot.get_u64().1)
    }

    #[napi]
    pub fn check(&self, slot: BigInt) -> SlotHistoryCheck {
        SlotHistoryCheck::from(self.0.check(slot.get_u64().1))
    }

    #[napi]
    pub fn oldest(&self) -> u64 {
        self.0.oldest()
    }

    #[napi]
    pub fn newest(&self) -> u64 {
        self.0.newest()
    }

    #[napi(js_name = "toString")]
    pub fn js_to_string(&self) -> String {
        format!("{self:?}")
    }
}
