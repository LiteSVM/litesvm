use {
    agave_feature_set::FeatureSet,
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    solana_account::AccountSharedData,
    solana_address::Address,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_fee_structure::{FeeBin, FeeStructure},
    solana_hash::Hash,
    solana_message::{
        compiled_instruction::CompiledInstruction,
        inner_instruction::{InnerInstruction, InnerInstructionsList},
    },
    solana_signature::Signature,
    solana_transaction_context::transaction::TransactionReturnData,
    solana_transaction_error::TransactionError,
    wincode::{SchemaRead, SchemaWrite},
};

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "FeeBin")]
pub(crate) struct FeeBinWire {
    pub limit: u64,
    pub fee: u64,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "FeeStructure")]
pub(crate) struct FeeStructureWire {
    pub lamports_per_signature: u64,
    pub lamports_per_write_lock: u64,
    #[wincode(with = "Vec<FeeBinWire>")]
    pub compute_fee_bins: Vec<FeeBin>,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "ComputeBudget")]
pub(crate) struct ComputeBudgetWire {
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
    pub alt_bn128_g1_addition_cost: u64,
    pub alt_bn128_g2_addition_cost: u64,
    pub alt_bn128_g1_multiplication_cost: u64,
    pub alt_bn128_g2_multiplication_cost: u64,
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
    pub bls12_381_g1_add_cost: u64,
    pub bls12_381_g2_add_cost: u64,
    pub bls12_381_g1_subtract_cost: u64,
    pub bls12_381_g2_subtract_cost: u64,
    pub bls12_381_g1_multiply_cost: u64,
    pub bls12_381_g2_multiply_cost: u64,
    pub bls12_381_g1_decompress_cost: u64,
    pub bls12_381_g2_decompress_cost: u64,
    pub bls12_381_g1_validate_cost: u64,
    pub bls12_381_g2_validate_cost: u64,
    pub bls12_381_one_pair_cost: u64,
    pub bls12_381_additional_pair_cost: u64,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "InnerInstruction")]
pub(crate) struct InnerInstructionWire {
    pub instruction: CompiledInstruction,
    pub stack_height: u8,
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FeatureActivationWire {
    pub address: Address,
    pub slot: u64,
}

impl From<(Address, u64)> for FeatureActivationWire {
    fn from((address, slot): (Address, u64)) -> Self {
        Self { address, slot }
    }
}

impl From<FeatureActivationWire> for (Address, u64) {
    fn from(entry: FeatureActivationWire) -> Self {
        (entry.address, entry.slot)
    }
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "TransactionMetadata")]
pub(crate) struct TransactionMetadataWire {
    pub signature: Signature,
    pub logs: Vec<String>,
    #[wincode(with = "Vec<Vec<InnerInstructionWire>>")]
    pub inner_instructions: InnerInstructionsList,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
    pub fee: u64,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "FailedTransactionMetadata")]
pub(crate) struct FailedTransactionMetadataWire {
    pub err: TransactionError,
    #[wincode(with = "TransactionMetadataWire")]
    pub meta: TransactionMetadata,
}

/// Mirror of `Result<TransactionMetadata, FailedTransactionMetadata>` so
/// wincode can derive a schema for it.
#[derive(SchemaWrite, SchemaRead)]
pub(crate) enum TxResult {
    Ok(#[wincode(with = "TransactionMetadataWire")] TransactionMetadata),
    Err(#[wincode(with = "FailedTransactionMetadataWire")] FailedTransactionMetadata),
}

impl TxResult {
    pub fn from_result(r: TransactionResult) -> Self {
        match r {
            Ok(m) => TxResult::Ok(m),
            Err(e) => TxResult::Err(e),
        }
    }

    pub fn into_result(self) -> TransactionResult {
        match self {
            TxResult::Ok(m) => Ok(m),
            TxResult::Err(e) => Err(e),
        }
    }
}

// ── FeatureSet snapshot (uses AHashMap/AHashSet, can't use serde remote) ──

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FeatureSetSnapshot {
    pub active: Vec<FeatureActivationWire>,
    pub inactive: Vec<Address>,
}

impl FeatureSetSnapshot {
    pub fn from_feature_set(fs: &FeatureSet) -> Self {
        let active = fs
            .active()
            .iter()
            .map(|(k, v)| FeatureActivationWire::from((*k, *v)))
            .collect();
        let inactive = fs.inactive().iter().copied().collect();
        Self { active, inactive }
    }

    pub fn into_feature_set(self) -> FeatureSet {
        FeatureSet::new(
            self.active.into_iter().map(Into::into).collect(),
            self.inactive.into_iter().collect(),
        )
    }
}

// ── Top-level snapshot ─────────────────────────────────────────────────

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct AccountEntryWire {
    pub address: Address,
    pub account: AccountSharedData,
}

impl From<(Address, AccountSharedData)> for AccountEntryWire {
    fn from((address, account): (Address, AccountSharedData)) -> Self {
        Self { address, account }
    }
}

impl From<AccountEntryWire> for (Address, AccountSharedData) {
    fn from(entry: AccountEntryWire) -> Self {
        (entry.address, entry.account)
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct LiteSvmSnapshot {
    pub accounts: Vec<AccountEntryWire>,
    pub airdrop_kp: [u8; 64],
    pub feature_set: FeatureSetSnapshot,
    pub latest_blockhash: Hash,
    pub history: Vec<(Signature, TxResult)>,
    pub history_capacity: u64,
    #[wincode(with = "Option<ComputeBudgetWire>")]
    pub compute_budget: Option<ComputeBudget>,
    pub sigverify: bool,
    pub blockhash_check: bool,
    #[wincode(with = "FeeStructureWire")]
    pub fee_structure: FeeStructure,
    pub log_bytes_limit: Option<u64>,
}
