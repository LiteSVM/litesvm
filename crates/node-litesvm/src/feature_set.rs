use {
    crate::{
        to_string_js,
        util::{bigint_to_u64, convert_pubkey},
    },
    agave_feature_set::FeatureSet as FeatureSetOriginal,
    napi::bindgen_prelude::*,
};

#[derive(Debug, Clone)]
#[napi]
pub struct FeatureSet(pub(crate) FeatureSetOriginal);

#[napi]
impl FeatureSet {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self(FeatureSetOriginal::default())
    }

    #[napi(factory)]
    pub fn all_enabled() -> Self {
        Self(FeatureSetOriginal::all_enabled())
    }

    #[napi]
    pub fn is_active(&self, feature_id: &[u8]) -> bool {
        self.0.is_active(&convert_pubkey(feature_id))
    }

    #[napi]
    pub fn activated_slot(&self, feature_id: &[u8]) -> Option<u64> {
        self.0.activated_slot(&convert_pubkey(feature_id))
    }

    #[napi]
    pub fn activate(&mut self, feature_id: &[u8], slot: BigInt) {
        let slot_u64 = bigint_to_u64(&slot).unwrap_or(0);
        self.0.activate(&convert_pubkey(feature_id), slot_u64);
    }

    #[napi]
    pub fn deactivate(&mut self, feature_id: &[u8]) {
        self.0.deactivate(&convert_pubkey(feature_id));
    }

    #[napi]
    pub fn get_active_features(&self) -> Vec<Buffer> {
        self.0
            .active()
            .keys()
            .map(|pubkey| Buffer::from(pubkey.to_bytes().to_vec()))
            .collect()
    }

    #[napi]
    pub fn get_inactive_features(&self) -> Vec<Buffer> {
        self.0
            .inactive()
            .iter()
            .map(|pubkey| Buffer::from(pubkey.to_bytes().to_vec()))
            .collect()
    }

    #[napi]
    pub fn get_active_features_count(&self) -> u32 {
        self.0.active().len() as u32
    }

    #[napi]
    pub fn get_inactive_features_count(&self) -> u32 {
        self.0.inactive().len() as u32
    }
}

to_string_js!(FeatureSet);
