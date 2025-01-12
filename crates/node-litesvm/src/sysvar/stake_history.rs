use {
    crate::to_string_js,
    napi::bindgen_prelude::*,
    solana_sdk::stake_history::{
        StakeHistory as StakeHistoryOriginal, StakeHistoryEntry as StakeHistoryEntryOriginal,
    },
};

#[derive(Debug, Clone)]
#[napi]
pub struct StakeHistoryEntry(pub(crate) StakeHistoryEntryOriginal);

#[napi]
impl StakeHistoryEntry {
    /// @param effective - effective stake at this epoch
    /// @param activating - sum of portion of stakes not fully warmed up
    /// @param effective - requested to be cooled down, not fully deactivated yet
    #[napi(constructor)]
    pub fn new(effective: BigInt, activating: BigInt, deactivating: BigInt) -> Self {
        Self(StakeHistoryEntryOriginal {
            effective: effective.get_u64().1,
            activating: activating.get_u64().1,
            deactivating: deactivating.get_u64().1,
        })
    }

    /// effective stake at this epoch
    #[napi(getter)]
    pub fn effective(&self) -> u64 {
        self.0.effective
    }

    #[napi(setter)]
    pub fn set_effective(&mut self, val: BigInt) {
        self.0.effective = val.get_u64().1
    }

    /// sum of portion of stakes not fully warmed up
    #[napi(getter)]
    pub fn activating(&self) -> u64 {
        self.0.activating
    }

    #[napi(setter)]
    pub fn set_activating(&mut self, val: BigInt) {
        self.0.effective = val.get_u64().1
    }

    /// requested to be cooled down, not fully deactivated yet
    #[napi(getter)]
    pub fn deactivating(&self) -> u64 {
        self.0.deactivating
    }

    #[napi(setter)]
    pub fn set_deactivating(&mut self, val: BigInt) {
        self.0.effective = val.get_u64().1
    }
}

to_string_js!(StakeHistoryEntry);

/// A type to hold data for the StakeHistory sysvar.
#[derive(Debug)]
#[napi]
pub struct StakeHistory(pub(crate) StakeHistoryOriginal);

#[napi]
impl StakeHistory {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self(StakeHistoryOriginal::default())
    }

    #[napi]
    pub fn get(&self, epoch: BigInt) -> Option<StakeHistoryEntry> {
        self.0
            .get(epoch.get_u64().1)
            .map(|x| StakeHistoryEntry(x.clone()))
    }

    #[napi]
    pub fn add(&mut self, epoch: BigInt, entry: &StakeHistoryEntry) {
        self.0.add(epoch.get_u64().1, entry.clone().0);
    }
}
