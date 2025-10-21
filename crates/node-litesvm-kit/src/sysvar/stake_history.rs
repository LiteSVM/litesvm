use {
    crate::{to_string_js, util::bigint_to_u64},
    napi::bindgen_prelude::*,
    solana_stake_interface::stake_history::{
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
    pub fn new(effective: BigInt, activating: BigInt, deactivating: BigInt) -> Result<Self> {
        Ok(Self(StakeHistoryEntryOriginal {
            effective: bigint_to_u64(&effective)?,
            activating: bigint_to_u64(&activating)?,
            deactivating: bigint_to_u64(&deactivating)?,
        }))
    }

    /// effective stake at this epoch
    #[napi(getter)]
    pub fn effective(&self) -> u64 {
        self.0.effective
    }

    #[napi(setter)]
    pub fn set_effective(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.effective = bigint_to_u64(&val)?)
    }

    /// sum of portion of stakes not fully warmed up
    #[napi(getter)]
    pub fn activating(&self) -> u64 {
        self.0.activating
    }

    #[napi(setter)]
    pub fn set_activating(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.effective = bigint_to_u64(&val)?)
    }

    /// requested to be cooled down, not fully deactivated yet
    #[napi(getter)]
    pub fn deactivating(&self) -> u64 {
        self.0.deactivating
    }

    #[napi(setter)]
    pub fn set_deactivating(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.effective = bigint_to_u64(&val)?)
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
    pub fn get(&self, epoch: BigInt) -> Result<Option<StakeHistoryEntry>> {
        Ok(self
            .0
            .get(bigint_to_u64(&epoch)?)
            .map(|x| StakeHistoryEntry(x.clone())))
    }

    #[napi]
    pub fn add(&mut self, epoch: BigInt, entry: &StakeHistoryEntry) -> Result<()> {
        Ok(self.0.add(bigint_to_u64(&epoch)?, entry.clone().0))
    }
}
