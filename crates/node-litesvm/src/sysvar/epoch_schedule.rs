use {
    napi::bindgen_prelude::*, solana_sdk::epoch_schedule::EpochSchedule as EpochScheduleOriginal,
};

/// Configuration for epochs and slots.
#[derive(Debug)]
#[napi]
pub struct EpochSchedule(pub(crate) EpochScheduleOriginal);

#[napi]
impl EpochSchedule {
    /// @param slots_per_epoch - The maximum number of slots in each epoch.
    /// @param leader_schedule_slot_offset - A number of slots before beginning of an epoch to calculate a leader schedule for that epoch.
    /// @param warmup - Whether epochs start short and grow.
    /// @param first_normal_epoch - The first epoch after the warmup period.
    /// @param first_normal_slot - The first slot after the warmup period.
    #[napi(constructor)]
    pub fn new(
        slots_per_epoch: BigInt,
        leader_schedule_slot_offset: BigInt,
        warmup: bool,
        first_normal_epoch: BigInt,
        first_normal_slot: BigInt,
    ) -> Self {
        Self(EpochScheduleOriginal {
            slots_per_epoch: slots_per_epoch.get_u64().1,
            leader_schedule_slot_offset: leader_schedule_slot_offset.get_u64().1,
            warmup,
            first_normal_epoch: first_normal_epoch.get_u64().1,
            first_normal_slot: first_normal_slot.get_u64().1,
        })
    }

    /// The maximum number of slots in each epoch.
    #[napi(getter)]
    pub fn slots_per_epoch(&self) -> u64 {
        self.0.slots_per_epoch
    }

    /// A number of slots before beginning of an epoch to calculate
    /// a leader schedule for that epoch.
    #[napi(getter)]
    pub fn leader_schedule_slot_offset(&self) -> u64 {
        self.0.leader_schedule_slot_offset
    }

    /// Whether epochs start short and grow.
    #[napi(getter)]
    pub fn warmup(&self) -> bool {
        self.0.warmup
    }

    /// The first epoch after the warmup period.
    ///
    /// Basically: `log2(slots_per_epoch) - log2(MINIMUM_SLOTS_PER_EPOCH)`.
    #[napi(getter)]
    pub fn first_normal_epoch(&self) -> u64 {
        self.0.first_normal_epoch
    }

    /// The first slot after the warmup period.
    ///
    /// Basically: `MINIMUM_SLOTS_PER_EPOCH * (2.pow(first_normal_epoch) - 1)`.
    #[napi(getter)]
    pub fn first_normal_slot(&self) -> u64 {
        self.0.first_normal_slot
    }
}
