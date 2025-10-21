use {
    crate::util::{bigint_to_u128, bigint_to_u64, try_parse_hash},
    napi::bindgen_prelude::*,
    solana_epoch_rewards::EpochRewards as EpochRewardsOriginal,
};

/// A type to hold data for the EpochRewards sysvar.
#[derive(Debug)]
#[napi]
pub struct EpochRewards(pub(crate) EpochRewardsOriginal);

#[napi]
impl EpochRewards {
    /// @param distribution_starting_block_height - The starting block height of the rewards distribution in the current epoch
    /// @param num_partitions - Number of partitions in the rewards distribution in the current epoch
    /// @param parent_blockhash - The blockhash of the parent block of the first block in the epoch
    /// @param total_points - The total rewards points calculated for the current epoch
    /// @param total_rewards - The total rewards calculated for the current epoch
    /// @param distributed_rewards - The rewards currently distributed for the current epoch, in lamports
    /// @param active - Whether the rewards period (including calculation and distribution) is active
    #[napi(constructor)]
    pub fn new(
        distribution_starting_block_height: BigInt,
        num_partitions: BigInt,
        parent_blockhash: String,
        total_points: BigInt,
        total_rewards: BigInt,
        distributed_rewards: BigInt,
        active: bool,
    ) -> Result<Self> {
        let hash_parsed = try_parse_hash(&parent_blockhash)?;
        Ok(Self(EpochRewardsOriginal {
            distribution_starting_block_height: bigint_to_u64(&distribution_starting_block_height)?,
            num_partitions: bigint_to_u64(&num_partitions)?,
            parent_blockhash: hash_parsed,
            total_points: bigint_to_u128(&total_points)?,
            total_rewards: bigint_to_u64(&total_rewards)?,
            distributed_rewards: bigint_to_u64(&distributed_rewards)?,
            active,
        }))
    }

    /// The starting block height of the rewards distribution in the current
    /// epoch
    #[napi(getter)]
    pub fn distribution_starting_block_height(&self) -> u64 {
        self.0.distribution_starting_block_height
    }

    #[napi(setter)]
    pub fn set_distribution_starting_block_height(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.distribution_starting_block_height = bigint_to_u64(&val)?)
    }

    /// Number of partitions in the rewards distribution in the current epoch,
    /// used to generate an EpochRewardsHasher
    #[napi(getter)]
    pub fn num_partitions(&self) -> u64 {
        self.0.num_partitions
    }

    #[napi(setter)]
    pub fn set_num_partitions(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.num_partitions = bigint_to_u64(&val)?)
    }

    /// The blockhash of the parent block of the first block in the epoch, used
    /// to seed an EpochRewardsHasher
    #[napi(getter)]
    pub fn parent_blockhash(&self) -> String {
        self.0.parent_blockhash.to_string()
    }

    #[napi(setter)]
    pub fn set_parent_blockhash(&mut self, val: String) -> Result<()> {
        let hash_parsed = try_parse_hash(&val)?;
        self.0.parent_blockhash = hash_parsed;
        Ok(())
    }

    /// The total rewards points calculated for the current epoch, where points
    /// equals the sum of (delegated stake * credits observed) for all
    /// delegations
    #[napi(getter)]
    pub fn total_points(&self) -> u128 {
        self.0.total_points
    }

    #[napi(setter)]
    pub fn set_total_points(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.total_points = bigint_to_u128(&val)?)
    }

    /// The total rewards calculated for the current epoch. This may be greater
    /// than the total `distributed_rewards` at the end of the rewards period,
    /// due to rounding and inability to deliver rewards smaller than 1 lamport.
    #[napi(getter)]
    pub fn total_rewards(&self) -> u64 {
        self.0.total_rewards
    }

    #[napi(setter)]
    pub fn set_total_rewards(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.total_rewards = bigint_to_u64(&val)?)
    }

    /// The rewards currently distributed for the current epoch, in lamports
    #[napi(getter)]
    pub fn distributed_rewards(&self) -> u64 {
        self.0.distributed_rewards
    }

    #[napi(setter)]
    pub fn set_distributed_rewards(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.distributed_rewards = bigint_to_u64(&val)?)
    }

    /// Whether the rewards period (including calculation and distribution) is
    /// active
    #[napi(getter)]
    pub fn active(&self) -> bool {
        self.0.active
    }

    #[napi(setter)]
    pub fn set_active(&mut self, val: bool) {
        self.0.active = val;
    }
}
