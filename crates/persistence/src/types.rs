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
    std::marker::PhantomData,
    wincode::{adapter::FromInto, SchemaRead, SchemaWrite},
};

/// Declares a wire mirror of a foreign type, serialized through [`FromInto`]
/// (wincode 0.6 dropped `#[wincode(from = ...)]`). Fields keep the target's
/// types; `[..expr]` fills target fields absent from an older layout.
macro_rules! wire {
    (
        $wire:ident => $target:ty $([..$rest:expr])? {
            $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)?
        }
    ) => {
        #[derive(SchemaWrite, SchemaRead)]
        pub(crate) struct $wire {
            $($(#[$attr])* pub $field: $ty,)*
        }

        impl From<&$target> for $wire {
            fn from(value: &$target) -> Self {
                Self { $($field: value.$field.clone(),)* }
            }
        }

        impl From<$wire> for $target {
            fn from(value: $wire) -> Self {
                Self { $($field: value.$field,)* $(..$rest)? }
            }
        }
    };
}

wire!(FeeBinWire => FeeBin {
    limit: u64,
    fee: u64,
});

wire!(FeeStructureWire => FeeStructure {
    lamports_per_signature: u64,
    lamports_per_write_lock: u64,
    #[wincode(with = "Vec<FromInto<FeeBinWire, FeeBin>>")]
    compute_fee_bins: Vec<FeeBin>,
});

// Compute-budget layout of Solana 4.3, also written by persistence version 1
// (LiteSVM v0.15.2).
wire!(ComputeBudgetWire => ComputeBudget {
    compute_unit_limit: u64,
    log_64_units: u64,
    create_program_address_units: u64,
    invoke_units: u64,
    max_instruction_stack_depth: usize,
    max_instruction_trace_length: usize,
    sha256_base_cost: u64,
    sha256_byte_cost: u64,
    sha256_max_slices: u64,
    max_call_depth: usize,
    stack_frame_size: usize,
    log_pubkey_units: u64,
    cpi_bytes_per_unit: u64,
    sysvar_base_cost: u64,
    secp256k1_recover_cost: u64,
    syscall_base_cost: u64,
    curve25519_edwards_validate_point_cost: u64,
    curve25519_edwards_add_cost: u64,
    curve25519_edwards_subtract_cost: u64,
    curve25519_edwards_multiply_cost: u64,
    curve25519_edwards_msm_base_cost: u64,
    curve25519_edwards_msm_incremental_cost: u64,
    curve25519_ristretto_validate_point_cost: u64,
    curve25519_ristretto_add_cost: u64,
    curve25519_ristretto_subtract_cost: u64,
    curve25519_ristretto_multiply_cost: u64,
    curve25519_ristretto_msm_base_cost: u64,
    curve25519_ristretto_msm_incremental_cost: u64,
    heap_size: u32,
    heap_cost: u64,
    mem_op_base_cost: u64,
    alt_bn128_g1_addition_cost: u64,
    alt_bn128_g2_addition_cost: u64,
    alt_bn128_g1_multiplication_cost: u64,
    alt_bn128_g2_multiplication_cost: u64,
    alt_bn128_pairing_one_pair_cost_first: u64,
    alt_bn128_pairing_one_pair_cost_other: u64,
    big_modular_exponentiation_base_cost: u64,
    big_modular_exponentiation_cost_divisor: u64,
    poseidon_cost_coefficient_a: u64,
    poseidon_cost_coefficient_c: u64,
    get_remaining_compute_units_cost: u64,
    alt_bn128_g1_compress: u64,
    alt_bn128_g1_decompress: u64,
    alt_bn128_g2_compress: u64,
    alt_bn128_g2_decompress: u64,
    bls12_381_g1_add_cost: u64,
    bls12_381_g2_add_cost: u64,
    bls12_381_g1_subtract_cost: u64,
    bls12_381_g2_subtract_cost: u64,
    bls12_381_g1_multiply_cost: u64,
    bls12_381_g2_multiply_cost: u64,
    bls12_381_g1_decompress_cost: u64,
    bls12_381_g2_decompress_cost: u64,
    bls12_381_g1_validate_cost: u64,
    bls12_381_g2_validate_cost: u64,
    bls12_381_one_pair_cost: u64,
    bls12_381_additional_pair_cost: u64,
});

// Compute-budget layout written by persistence versions 2 and 3 (Solana 4.2),
// which lacked the two modular-exponentiation fields (filled with defaults).
wire!(ComputeBudgetV2Wire => ComputeBudget [..ComputeBudget::new_with_defaults(false)] {
    compute_unit_limit: u64,
    log_64_units: u64,
    create_program_address_units: u64,
    invoke_units: u64,
    max_instruction_stack_depth: usize,
    max_instruction_trace_length: usize,
    sha256_base_cost: u64,
    sha256_byte_cost: u64,
    sha256_max_slices: u64,
    max_call_depth: usize,
    stack_frame_size: usize,
    log_pubkey_units: u64,
    cpi_bytes_per_unit: u64,
    sysvar_base_cost: u64,
    secp256k1_recover_cost: u64,
    syscall_base_cost: u64,
    curve25519_edwards_validate_point_cost: u64,
    curve25519_edwards_add_cost: u64,
    curve25519_edwards_subtract_cost: u64,
    curve25519_edwards_multiply_cost: u64,
    curve25519_edwards_msm_base_cost: u64,
    curve25519_edwards_msm_incremental_cost: u64,
    curve25519_ristretto_validate_point_cost: u64,
    curve25519_ristretto_add_cost: u64,
    curve25519_ristretto_subtract_cost: u64,
    curve25519_ristretto_multiply_cost: u64,
    curve25519_ristretto_msm_base_cost: u64,
    curve25519_ristretto_msm_incremental_cost: u64,
    heap_size: u32,
    heap_cost: u64,
    mem_op_base_cost: u64,
    alt_bn128_g1_addition_cost: u64,
    alt_bn128_g2_addition_cost: u64,
    alt_bn128_g1_multiplication_cost: u64,
    alt_bn128_g2_multiplication_cost: u64,
    alt_bn128_pairing_one_pair_cost_first: u64,
    alt_bn128_pairing_one_pair_cost_other: u64,
    poseidon_cost_coefficient_a: u64,
    poseidon_cost_coefficient_c: u64,
    get_remaining_compute_units_cost: u64,
    alt_bn128_g1_compress: u64,
    alt_bn128_g1_decompress: u64,
    alt_bn128_g2_compress: u64,
    alt_bn128_g2_decompress: u64,
    bls12_381_g1_add_cost: u64,
    bls12_381_g2_add_cost: u64,
    bls12_381_g1_subtract_cost: u64,
    bls12_381_g2_subtract_cost: u64,
    bls12_381_g1_multiply_cost: u64,
    bls12_381_g2_multiply_cost: u64,
    bls12_381_g1_decompress_cost: u64,
    bls12_381_g2_decompress_cost: u64,
    bls12_381_g1_validate_cost: u64,
    bls12_381_g2_validate_cost: u64,
    bls12_381_one_pair_cost: u64,
    bls12_381_additional_pair_cost: u64,
});

wire!(InnerInstructionWire => InnerInstruction {
    instruction: CompiledInstruction,
    stack_height: u8,
});

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

wire!(TransactionMetadataWire => TransactionMetadata {
    signature: Signature,
    logs: Vec<String>,
    #[wincode(with = "Vec<Vec<FromInto<InnerInstructionWire, InnerInstruction>>>")]
    inner_instructions: InnerInstructionsList,
    compute_units_consumed: u64,
    return_data: TransactionReturnData,
    fee: u64,
});

wire!(FailedTransactionMetadataWire => FailedTransactionMetadata {
    err: TransactionError,
    #[wincode(with = "FromInto<TransactionMetadataWire, TransactionMetadata>")]
    meta: TransactionMetadata,
});

/// Mirror of `Result<TransactionMetadata, FailedTransactionMetadata>` so
/// wincode can derive a schema for it.
#[derive(SchemaWrite, SchemaRead)]
pub(crate) enum TxResult {
    Ok(
        #[wincode(with = "FromInto<TransactionMetadataWire, TransactionMetadata>")]
        TransactionMetadata,
    ),
    Err(
        #[wincode(with = "FromInto<FailedTransactionMetadataWire, FailedTransactionMetadata>")]
        FailedTransactionMetadata,
    ),
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

/// LiteSVM state; `B` is the compute-budget wire layout. Version 1 and the
/// current version share [`ComputeBudgetWire`], versions 2 and 3 use
/// [`ComputeBudgetV2Wire`].
#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct LiteSvmState<B = ComputeBudgetWire> {
    pub accounts: Vec<AccountEntryWire>,
    pub airdrop_kp: [u8; 64],
    pub feature_set: FeatureSetSnapshot,
    pub latest_blockhash: Hash,
    pub history: Vec<(Signature, TxResult)>,
    pub history_capacity: u64,
    #[wincode(with = "Option<FromInto<B, ComputeBudget>>")]
    pub compute_budget: Option<ComputeBudget>,
    pub sigverify: bool,
    pub blockhash_check: bool,
    #[wincode(with = "FromInto<FeeStructureWire, FeeStructure>")]
    pub fee_structure: FeeStructure,
    pub log_bytes_limit: Option<u64>,
    #[wincode(skip)]
    pub budget_layout: PhantomData<B>,
}

impl<B> LiteSvmState<B> {
    /// Switches the compute-budget wire layout; the in-memory state is unchanged.
    pub fn with_layout<L>(self) -> LiteSvmState<L> {
        LiteSvmState {
            accounts: self.accounts,
            airdrop_kp: self.airdrop_kp,
            feature_set: self.feature_set,
            latest_blockhash: self.latest_blockhash,
            history: self.history,
            history_capacity: self.history_capacity,
            compute_budget: self.compute_budget,
            sigverify: self.sigverify,
            blockhash_check: self.blockhash_check,
            fee_structure: self.fee_structure,
            log_bytes_limit: self.log_bytes_limit,
            budget_layout: PhantomData,
        }
    }
}

/// Snapshot written by persistence version 3 onwards.
#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct LiteSvmSnapshot<B = ComputeBudgetWire> {
    pub state: LiteSvmState<B>,
    pub epoch_vote_stakes: Vec<(Address, u64)>,
}

impl<B> LiteSvmSnapshot<B> {
    pub fn with_layout<L>(self) -> LiteSvmSnapshot<L> {
        LiteSvmSnapshot {
            state: self.state.with_layout(),
            epoch_vote_stakes: self.epoch_vote_stakes,
        }
    }
}

impl From<LiteSvmState> for LiteSvmSnapshot {
    fn from(state: LiteSvmState) -> Self {
        Self {
            state,
            epoch_vote_stakes: Vec::new(),
        }
    }
}
