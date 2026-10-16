use {
    crate::util::bigint_to_u64, napi::bindgen_prelude::*,
    solana_epoch_schedule::EpochSchedule as EpochScheduleOriginal,
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
    ) -> Result<Self> {
        Ok(Self(EpochScheduleOriginal {
            slots_per_epoch: bigint_to_u64(&slots_per_epoch)?,
            leader_schedule_slot_offset: bigint_to_u64(&leader_schedule_slot_offset)?,
            warmup,
            first_normal_epoch: bigint_to_u64(&first_normal_epoch)?,
            first_normal_slot: bigint_to_u64(&first_normal_slot)?,
        }))
    }

    /// The maximum number of slots in each epoch.
    #[napi(getter)]
    pub fn slots_per_epoch(&self) -> u64 {
        self.0.slots_per_epoch
    }

    #[napi(setter)]
    pub fn set_slots_per_epoch(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.slots_per_epoch = bigint_to_u64(&val)?)
    }

    /// A number of slots before beginning of an epoch to calculate
    /// a leader schedule for that epoch.
    #[napi(getter)]
    pub fn leader_schedule_slot_offset(&self) -> u64 {
        self.0.leader_schedule_slot_offset
    }

    #[napi(setter)]
    pub fn set_leader_schedule_slot_offset(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.leader_schedule_slot_offset = bigint_to_u64(&val)?)
    }

    /// Whether epochs start short and grow.
    #[napi(getter)]
    pub fn warmup(&self) -> bool {
        self.0.warmup
    }

    #[napi(setter)]
    pub fn set_warmup(&mut self, val: bool) {
        self.0.warmup = val;
    }

    /// The first epoch after the warmup period.
    ///
    /// Basically: `log2(slots_per_epoch) - log2(MINIMUM_SLOTS_PER_EPOCH)`.
    #[napi(getter)]
    pub fn first_normal_epoch(&self) -> u64 {
        self.0.first_normal_epoch
    }

    #[napi(setter)]
    pub fn set_first_normal_epoch(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.first_normal_epoch = bigint_to_u64(&val)?)
    }

    /// The first slot after the warmup period.
    ///
    /// Basically: `MINIMUM_SLOTS_PER_EPOCH * (2.pow(first_normal_epoch) - 1)`.
    #[napi(getter)]
    pub fn first_normal_slot(&self) -> u64 {
        self.0.first_normal_slot
    }

    #[napi(setter)]
    pub fn set_first_normal_slot(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.first_normal_slot = bigint_to_u64(&val)?)
    }
}
