use {
    agave_feature_set::FeatureSet,
    litesvm::types::TransactionResult,
    serde::{Deserialize, Serialize},
    solana_account::AccountSharedData,
    solana_address::Address,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_fee_structure::{FeeBin, FeeStructure},
    solana_hash::Hash,
    solana_signature::Signature,
};

// ── Serde remote definitions for upstream types without serde ──────────

#[derive(Serialize, Deserialize)]
#[serde(remote = "FeeBin")]
struct FeeBinDef {
    pub limit: u64,
    pub fee: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "FeeStructure")]
struct FeeStructureDef {
    pub lamports_per_signature: u64,
    pub lamports_per_write_lock: u64,
    #[serde(with = "fee_bin_vec")]
    pub compute_fee_bins: Vec<FeeBin>,
}

/// Helper module to serialize `Vec<FeeBin>` using the remote definition.
mod fee_bin_vec {
    use super::*;
    use serde::{Deserializer, Serializer};

    #[derive(Serialize, Deserialize)]
    struct FeeBinWrapper(#[serde(with = "FeeBinDef")] FeeBin);

    pub fn serialize<S: Serializer>(v: &[FeeBin], s: S) -> Result<S::Ok, S::Error> {
        let wrappers: Vec<FeeBinWrapper> = v.iter().map(|b| FeeBinWrapper(b.clone())).collect();
        wrappers.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<FeeBin>, D::Error> {
        let wrappers = Vec::<FeeBinWrapper>::deserialize(d)?;
        Ok(wrappers.into_iter().map(|w| w.0).collect())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ComputeBudget")]
struct ComputeBudgetDef {
    pub compute_unit_limit: u64,
    pub log_64_units: u64,
    pub create_program_address_units: u64,
    pub invoke_units: u64,
    pub max_instruction_stack_depth: usize,
    pub max_instruction_trace_length: usize,
    pub sha256_base_cost: u64,
    pub sha256_byte_cost: u64,
    pub sha256_max_slices: u64,
    pub max_call_depth: usize,
    pub stack_frame_size: usize,
    pub log_pubkey_units: u64,
    pub cpi_bytes_per_unit: u64,
    pub sysvar_base_cost: u64,
    pub secp256k1_recover_cost: u64,
    pub syscall_base_cost: u64,
    pub curve25519_edwards_validate_point_cost: u64,
    pub curve25519_edwards_add_cost: u64,
    pub curve25519_edwards_subtract_cost: u64,
    pub curve25519_edwards_multiply_cost: u64,
    pub curve25519_edwards_msm_base_cost: u64,
    pub curve25519_edwards_msm_incremental_cost: u64,
    pub curve25519_ristretto_validate_point_cost: u64,
    pub curve25519_ristretto_add_cost: u64,
    pub curve25519_ristretto_subtract_cost: u64,
    pub curve25519_ristretto_multiply_cost: u64,
    pub curve25519_ristretto_msm_base_cost: u64,
    pub curve25519_ristretto_msm_incremental_cost: u64,
    pub heap_size: u32,
    pub heap_cost: u64,
    pub mem_op_base_cost: u64,
    pub alt_bn128_addition_cost: u64,
    pub alt_bn128_multiplication_cost: u64,
    pub alt_bn128_pairing_one_pair_cost_first: u64,
    pub alt_bn128_pairing_one_pair_cost_other: u64,
    pub big_modular_exponentiation_base_cost: u64,
    pub big_modular_exponentiation_cost_divisor: u64,
    pub poseidon_cost_coefficient_a: u64,
    pub poseidon_cost_coefficient_c: u64,
    pub get_remaining_compute_units_cost: u64,
    pub alt_bn128_g1_compress: u64,
    pub alt_bn128_g1_decompress: u64,
    pub alt_bn128_g2_compress: u64,
    pub alt_bn128_g2_decompress: u64,
}

/// Helper module to serialize `Option<ComputeBudget>` using the remote definition.
mod compute_budget_option {
    use super::*;
    use serde::{Deserializer, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Wrapper(#[serde(with = "ComputeBudgetDef")] ComputeBudget);

    pub fn serialize<S: Serializer>(v: &Option<ComputeBudget>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(|cb| Wrapper(*cb)).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<ComputeBudget>, D::Error> {
        let wrapper = Option::<Wrapper>::deserialize(d)?;
        Ok(wrapper.map(|w| w.0))
    }
}

// ── FeatureSet snapshot (uses AHashMap/AHashSet, can't use serde remote) ──

#[derive(Serialize, Deserialize)]
pub(crate) struct FeatureSetSnapshot {
    pub active: Vec<(Address, u64)>,
    pub inactive: Vec<Address>,
}

impl FeatureSetSnapshot {
    pub fn from_feature_set(fs: &FeatureSet) -> Self {
        let mut active: Vec<(Address, u64)> = fs.active().iter().map(|(k, v)| (*k, *v)).collect();
        active.sort_by_key(|(k, _)| *k);
        let mut inactive: Vec<Address> = fs.inactive().iter().copied().collect();
        inactive.sort();
        Self { active, inactive }
    }

    pub fn into_feature_set(self) -> FeatureSet {
        FeatureSet::new(
            self.active.into_iter().collect(),
            self.inactive.into_iter().collect(),
        )
    }
}

// ── Top-level snapshot ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct LiteSvmSnapshot {
    pub accounts: Vec<(Address, AccountSharedData)>,
    pub airdrop_kp: Vec<u8>,
    pub feature_set: FeatureSetSnapshot,
    pub latest_blockhash: Hash,
    pub history: Vec<(Signature, TransactionResult)>,
    pub history_capacity: usize,
    #[serde(with = "compute_budget_option")]
    pub compute_budget: Option<ComputeBudget>,
    pub sigverify: bool,
    pub blockhash_check: bool,
    #[serde(with = "FeeStructureDef")]
    pub fee_structure: FeeStructure,
    pub log_bytes_limit: Option<usize>,
}
