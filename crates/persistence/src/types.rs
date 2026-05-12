use {
    agave_feature_set::FeatureSet,
    core::mem::MaybeUninit,
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    solana_account::{Account, AccountSharedData, ReadableAccount},
    solana_address::Address,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_fee_structure::{FeeBin, FeeStructure},
    solana_hash::Hash,
    solana_message::{
        compiled_instruction::CompiledInstruction,
        inner_instruction::{InnerInstruction, InnerInstructionsList},
    },
    solana_signature::Signature,
    solana_transaction_context::TransactionReturnData,
    solana_transaction_error::TransactionError,
    wincode::{
        config::Config,
        error::{ReadResult, WriteResult},
        io::{Reader, Writer},
        SchemaRead, SchemaWrite,
    },
};

// ── POD wrappers for newtype byte arrays ───────────────────────────────

wincode::pod_wrapper! {
    unsafe struct PodHash(Hash);
    unsafe struct PodAirdropKp([u8; 64]);
}

// ── Wincode shadows for foreign types lacking upstream wincode ─────────

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "Account")]
pub(crate) struct AccountWire {
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Address,
    pub executable: bool,
    pub rent_epoch: u64,
}

/// Wincode schema for [`AccountSharedData`] that writes via accessors (avoiding the
/// `Vec<u8>` data clone) and reads through the public `Account` shape via
/// [`AccountWire`]. Wire format is identical to [`AccountWire`]/[`Account`].
pub(crate) struct AccountSchema;

unsafe impl<C: Config> SchemaWrite<C> for AccountSchema {
    type Src = AccountSharedData;

    fn size_of(src: &AccountSharedData) -> WriteResult<usize> {
        let lamports = src.lamports();
        let owner = *src.owner();
        let executable = src.executable();
        let rent_epoch = src.rent_epoch();
        Ok(<u64 as SchemaWrite<C>>::size_of(&lamports)?
            + <[u8] as SchemaWrite<C>>::size_of(src.data())?
            + <Address as SchemaWrite<C>>::size_of(&owner)?
            + <bool as SchemaWrite<C>>::size_of(&executable)?
            + <u64 as SchemaWrite<C>>::size_of(&rent_epoch)?)
    }

    fn write(mut writer: impl Writer, src: &AccountSharedData) -> WriteResult<()> {
        let lamports = src.lamports();
        let owner = *src.owner();
        let executable = src.executable();
        let rent_epoch = src.rent_epoch();
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &lamports)?;
        <[u8] as SchemaWrite<C>>::write(writer.by_ref(), src.data())?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &owner)?;
        <bool as SchemaWrite<C>>::write(writer.by_ref(), &executable)?;
        <u64 as SchemaWrite<C>>::write(writer, &rent_epoch)?;
        Ok(())
    }
}

unsafe impl<'de, C: Config> SchemaRead<'de, C> for AccountSchema {
    type Dst = AccountSharedData;

    fn read(reader: impl Reader<'de>, dst: &mut MaybeUninit<AccountSharedData>) -> ReadResult<()> {
        let mut account = MaybeUninit::<Account>::uninit();
        <AccountWire as SchemaRead<'de, C>>::read(reader, &mut account)?;
        // SAFETY: AccountWire::read fully initialized `account` on Ok.
        let account = unsafe { account.assume_init() };
        dst.write(account.into());
        Ok(())
    }
}

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

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "CompiledInstruction")]
pub(crate) struct CompiledInstructionWire {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "InnerInstruction")]
pub(crate) struct InnerInstructionWire {
    #[wincode(with = "CompiledInstructionWire")]
    pub instruction: CompiledInstruction,
    pub stack_height: u8,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "TransactionReturnData")]
pub(crate) struct TransactionReturnDataWire {
    pub program_id: Address,
    pub data: Vec<u8>,
}

#[derive(SchemaWrite, SchemaRead)]
#[wincode(from = "TransactionMetadata")]
pub(crate) struct TransactionMetadataWire {
    pub signature: Signature,
    pub logs: Vec<String>,
    #[wincode(with = "Vec<Vec<InnerInstructionWire>>")]
    pub inner_instructions: InnerInstructionsList,
    pub compute_units_consumed: u64,
    #[wincode(with = "TransactionReturnDataWire")]
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
    pub active: Vec<(Address, u64)>,
    pub inactive: Vec<Address>,
}

impl FeatureSetSnapshot {
    pub fn from_feature_set(fs: &FeatureSet) -> Self {
        let active = fs.active().iter().map(|(k, v)| (*k, *v)).collect();
        let inactive = fs.inactive().iter().copied().collect();
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

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct LiteSvmSnapshot {
    #[wincode(with = "Vec<(Address, AccountSchema)>")]
    pub accounts: Vec<(Address, AccountSharedData)>,
    #[wincode(with = "PodAirdropKp")]
    pub airdrop_kp: [u8; 64],
    pub feature_set: FeatureSetSnapshot,
    #[wincode(with = "PodHash")]
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
