use {
    crate::{to_string_js, util::convert_pubkey},
    napi::bindgen_prelude::*,
    solana_feature_set::FeatureSet as FeatureSetOriginal,
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
    pub fn is_active(&self, feature_id: Uint8Array) -> bool {
        self.0.is_active(&convert_pubkey(feature_id))
    }

    #[napi]
    pub fn activated_slot(&self, feature_id: Uint8Array) -> Option<u64> {
        self.0.activated_slot(&convert_pubkey(feature_id))
    }
}

to_string_js!(FeatureSet);
