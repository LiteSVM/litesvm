use {
    napi::bindgen_prelude::*,
    solana_sdk::{epoch_rewards::EpochRewards as EpochRewardsOriginal, hash::Hash},
    std::str::FromStr,
};

/// A type to hold data for the EpochRewards sysvar.
#[derive(Debug)]
#[napi]
pub struct EpochRewards(pub(crate) EpochRewardsOriginal);

fn try_parse_hash(raw: &str) -> Result<Hash> {
    Hash::from_str(raw).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to parse blockhash: {e}"),
        )
    })
}

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
            distribution_starting_block_height: distribution_starting_block_height.get_u64().1,
            num_partitions: num_partitions.get_u64().1,
            parent_blockhash: hash_parsed,
            total_points: total_points.get_u128().1,
            total_rewards: total_rewards.get_u64().1,
            distributed_rewards: distributed_rewards.get_u64().1,
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
    pub fn set_distribution_starting_block_height(&mut self, val: BigInt) {
        self.0.distribution_starting_block_height = val.get_u64().1;
    }

    /// Number of partitions in the rewards distribution in the current epoch,
    /// used to generate an EpochRewardsHasher
    #[napi(getter)]
    pub fn num_partitions(&self) -> u64 {
        self.0.num_partitions
    }

    #[napi(setter)]
    pub fn set_num_partitions(&mut self, val: BigInt) {
        self.0.num_partitions = val.get_u64().1;
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
    pub fn set_total_points(&mut self, val: BigInt) {
        self.0.total_points = val.get_u128().1;
    }

    /// The total rewards calculated for the current epoch. This may be greater
    /// than the total `distributed_rewards` at the end of the rewards period,
    /// due to rounding and inability to deliver rewards smaller than 1 lamport.
    #[napi(getter)]
    pub fn total_rewards(&self) -> u64 {
        self.0.total_rewards
    }

    #[napi(setter)]
    pub fn set_total_rewards(&mut self, val: BigInt) {
        self.0.total_rewards = val.get_u64().1;
    }

    /// The rewards currently distributed for the current epoch, in lamports
    #[napi(getter)]
    pub fn distributed_rewards(&self) -> u64 {
        self.0.distributed_rewards
    }

    #[napi(setter)]
    pub fn set_distributed_rewards(&mut self, val: BigInt) {
        self.0.distributed_rewards = val.get_u64().1;
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
