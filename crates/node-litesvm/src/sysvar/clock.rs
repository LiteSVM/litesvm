use {napi::bindgen_prelude::*, solana_sdk::clock::Clock as ClockOriginal};

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
        epoch_start_timestamp: i64,
        epoch: BigInt,
        leader_schedule_epoch: BigInt,
        unix_timestamp: i64,
    ) -> Self {
        Self(ClockOriginal {
            slot: slot.get_u64().1,
            epoch_start_timestamp,
            epoch: epoch.get_u64().1,
            leader_schedule_epoch: leader_schedule_epoch.get_u64().1,
            unix_timestamp,
        })
    }

    /// The current Slot.
    #[napi(getter)]
    pub fn slot(&self) -> u64 {
        self.0.slot
    }

    #[napi(setter)]
    pub fn set_slot(&mut self, val: BigInt) {
        self.0.slot = val.get_u64().1;
    }

    /// The current epoch.
    #[napi(getter)]
    pub fn epoch(&self) -> u64 {
        self.0.epoch
    }

    #[napi(setter)]
    pub fn set_epoch(&mut self, val: BigInt) {
        self.0.epoch = val.get_u64().1;
    }

    /// The timestamp of the first `Slot` in this `Epoch`.
    #[napi(getter)]
    pub fn epoch_start_timestamp(&self) -> i64 {
        self.0.epoch_start_timestamp
    }

    #[napi(setter)]
    pub fn set_epoch_start_timestamp(&mut self, val: i64) {
        self.0.epoch_start_timestamp = val;
    }

    /// The future Epoch for which the leader schedule has most recently been calculated.
    #[napi(getter)]
    pub fn leader_schedule_epoch(&self) -> u64 {
        self.0.leader_schedule_epoch
    }

    #[napi(setter)]
    pub fn set_leader_schedule_epoch(&mut self, val: BigInt) {
        self.0.leader_schedule_epoch = val.get_u64().1;
    }

    /// The approximate real world time of the current slot.
    #[napi(getter)]
    pub fn unix_timestamp(&self) -> i64 {
        self.0.unix_timestamp
    }

    #[napi(setter)]
    pub fn set_unix_timestamp(&mut self, val: i64) {
        self.0.unix_timestamp = val;
    }

    #[napi(js_name = "toString")]
    pub fn js_to_string(&self) -> String {
        format!("{self:?}")
    }
}
