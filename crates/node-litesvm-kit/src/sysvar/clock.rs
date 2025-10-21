use {
    crate::{
        to_string_js,
        util::{bigint_to_i64, bigint_to_u64},
    },
    napi::bindgen_prelude::*,
    solana_clock::Clock as ClockOriginal,
};

/// A representation of network time.
///
/// All members of `Clock` start from 0 upon network boot.
#[derive(Debug)]
#[napi]
pub struct Clock(pub(crate) ClockOriginal);

#[napi]
impl Clock {
    /// @param slot - The current Slot.
    /// @param epochStartTimestamp - The timestamp of the first `Slot` in this `Epoch`.
    /// @param epoch - The current epoch.
    /// @param leaderScheduleEpoch - The future Epoch for which the leader schedule has most recently been calculated.
    /// @param unixTimestamp - The approximate real world time of the current slot.
    #[napi(constructor)]
    pub fn new(
        slot: BigInt,
        epoch_start_timestamp: BigInt,
        epoch: BigInt,
        leader_schedule_epoch: BigInt,
        unix_timestamp: BigInt,
    ) -> Result<Self> {
        Ok(Self(ClockOriginal {
            slot: bigint_to_u64(&slot)?,
            epoch_start_timestamp: bigint_to_i64(&epoch_start_timestamp)?,
            epoch: bigint_to_u64(&epoch)?,
            leader_schedule_epoch: bigint_to_u64(&leader_schedule_epoch)?,
            unix_timestamp: bigint_to_i64(&unix_timestamp)?,
        }))
    }

    /// The current Slot.
    #[napi(getter)]
    pub fn slot(&self) -> u64 {
        self.0.slot
    }

    #[napi(setter)]
    pub fn set_slot(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.slot = bigint_to_u64(&val)?)
    }

    /// The current epoch.
    #[napi(getter)]
    pub fn epoch(&self) -> u64 {
        self.0.epoch
    }

    #[napi(setter)]
    pub fn set_epoch(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.epoch = bigint_to_u64(&val)?)
    }

    /// The timestamp of the first `Slot` in this `Epoch`.
    #[napi(getter)]
    pub fn epoch_start_timestamp(&self) -> BigInt {
        self.0.epoch_start_timestamp.into()
    }

    #[napi(setter)]
    pub fn set_epoch_start_timestamp(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.epoch_start_timestamp = bigint_to_i64(&val)?)
    }

    /// The future Epoch for which the leader schedule has most recently been calculated.
    #[napi(getter)]
    pub fn leader_schedule_epoch(&self) -> u64 {
        self.0.leader_schedule_epoch
    }

    #[napi(setter)]
    pub fn set_leader_schedule_epoch(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.leader_schedule_epoch = bigint_to_u64(&val)?)
    }

    /// The approximate real world time of the current slot.
    #[napi(getter)]
    pub fn unix_timestamp(&self) -> BigInt {
        self.0.unix_timestamp.into()
    }

    #[napi(setter)]
    pub fn set_unix_timestamp(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.unix_timestamp = bigint_to_i64(&val)?)
    }
}

to_string_js!(Clock);
