use {
    agave_feature_set::FeatureSet,
    litesvm::{
        types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
        LiteSVM,
    },
    serde::{Deserialize, Serialize},
    solana_account::{Account, AccountSharedData},
    solana_address::Address,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_fee_structure::FeeStructure,
    solana_hash::Hash,
    solana_signature::Signature,
    std::{
        fs,
        io::{self, BufReader, BufWriter, Read, Write},
        path::Path,
    },
    thiserror::Error,
};

fn crc32(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] bincode::Error),
    #[error("LiteSVM error: {0}")]
    LiteSvm(#[from] litesvm::error::LiteSVMError),
    #[error("Data integrity check failed (expected checksum {expected:#010x}, got {actual:#010x})")]
    ChecksumMismatch { expected: u32, actual: u32 },
}

/// Serializable wrapper for `TransactionResult` which is
/// `Result<TransactionMetadata, FailedTransactionMetadata>`.
#[derive(Serialize, Deserialize)]
enum TransactionResultState {
    Ok(TransactionMetadata),
    Err(FailedTransactionMetadata),
}

impl From<&TransactionResult> for TransactionResultState {
    fn from(result: &TransactionResult) -> Self {
        match result {
            Ok(meta) => TransactionResultState::Ok(meta.clone()),
            Err(meta) => TransactionResultState::Err(meta.clone()),
        }
    }
}

impl From<TransactionResultState> for TransactionResult {
    fn from(state: TransactionResultState) -> Self {
        match state {
            TransactionResultState::Ok(meta) => Ok(meta),
            TransactionResultState::Err(meta) => Err(meta),
        }
    }
}

/// Serializable mirror of `FeeBin` (upstream type lacks serde).
#[derive(Serialize, Deserialize)]
struct FeeBinState {
    limit: u64,
    fee: u64,
}

/// Serializable mirror of `FeeStructure` (upstream type lacks serde).
#[derive(Serialize, Deserialize)]
struct FeeStructureState {
    lamports_per_signature: u64,
    lamports_per_write_lock: u64,
    compute_fee_bins: Vec<FeeBinState>,
}

impl From<&FeeStructure> for FeeStructureState {
    fn from(fs: &FeeStructure) -> Self {
        Self {
            lamports_per_signature: fs.lamports_per_signature,
            lamports_per_write_lock: fs.lamports_per_write_lock,
            compute_fee_bins: fs
                .compute_fee_bins
                .iter()
                .map(|bin| FeeBinState {
                    limit: bin.limit,
                    fee: bin.fee,
                })
                .collect(),
        }
    }
}

impl From<FeeStructureState> for FeeStructure {
    fn from(state: FeeStructureState) -> Self {
        use solana_fee_structure::FeeBin;
        FeeStructure {
            lamports_per_signature: state.lamports_per_signature,
            lamports_per_write_lock: state.lamports_per_write_lock,
            compute_fee_bins: state
                .compute_fee_bins
                .into_iter()
                .map(|bin| FeeBin {
                    limit: bin.limit,
                    fee: bin.fee,
                })
                .collect(),
        }
    }
}

/// Serializable mirror of `ComputeBudget` (upstream type lacks serde).
#[derive(Serialize, Deserialize)]
struct ComputeBudgetState {
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
    alt_bn128_addition_cost: u64,
    alt_bn128_multiplication_cost: u64,
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
}

impl From<&ComputeBudget> for ComputeBudgetState {
    fn from(cb: &ComputeBudget) -> Self {
        Self {
            compute_unit_limit: cb.compute_unit_limit,
            log_64_units: cb.log_64_units,
            create_program_address_units: cb.create_program_address_units,
            invoke_units: cb.invoke_units,
            max_instruction_stack_depth: cb.max_instruction_stack_depth,
            max_instruction_trace_length: cb.max_instruction_trace_length,
            sha256_base_cost: cb.sha256_base_cost,
            sha256_byte_cost: cb.sha256_byte_cost,
            sha256_max_slices: cb.sha256_max_slices,
            max_call_depth: cb.max_call_depth,
            stack_frame_size: cb.stack_frame_size,
            log_pubkey_units: cb.log_pubkey_units,
            cpi_bytes_per_unit: cb.cpi_bytes_per_unit,
            sysvar_base_cost: cb.sysvar_base_cost,
            secp256k1_recover_cost: cb.secp256k1_recover_cost,
            syscall_base_cost: cb.syscall_base_cost,
            curve25519_edwards_validate_point_cost: cb.curve25519_edwards_validate_point_cost,
            curve25519_edwards_add_cost: cb.curve25519_edwards_add_cost,
            curve25519_edwards_subtract_cost: cb.curve25519_edwards_subtract_cost,
            curve25519_edwards_multiply_cost: cb.curve25519_edwards_multiply_cost,
            curve25519_edwards_msm_base_cost: cb.curve25519_edwards_msm_base_cost,
            curve25519_edwards_msm_incremental_cost: cb.curve25519_edwards_msm_incremental_cost,
            curve25519_ristretto_validate_point_cost: cb.curve25519_ristretto_validate_point_cost,
            curve25519_ristretto_add_cost: cb.curve25519_ristretto_add_cost,
            curve25519_ristretto_subtract_cost: cb.curve25519_ristretto_subtract_cost,
            curve25519_ristretto_multiply_cost: cb.curve25519_ristretto_multiply_cost,
            curve25519_ristretto_msm_base_cost: cb.curve25519_ristretto_msm_base_cost,
            curve25519_ristretto_msm_incremental_cost: cb
                .curve25519_ristretto_msm_incremental_cost,
            heap_size: cb.heap_size,
            heap_cost: cb.heap_cost,
            mem_op_base_cost: cb.mem_op_base_cost,
            alt_bn128_addition_cost: cb.alt_bn128_addition_cost,
            alt_bn128_multiplication_cost: cb.alt_bn128_multiplication_cost,
            alt_bn128_pairing_one_pair_cost_first: cb.alt_bn128_pairing_one_pair_cost_first,
            alt_bn128_pairing_one_pair_cost_other: cb.alt_bn128_pairing_one_pair_cost_other,
            big_modular_exponentiation_base_cost: cb.big_modular_exponentiation_base_cost,
            big_modular_exponentiation_cost_divisor: cb.big_modular_exponentiation_cost_divisor,
            poseidon_cost_coefficient_a: cb.poseidon_cost_coefficient_a,
            poseidon_cost_coefficient_c: cb.poseidon_cost_coefficient_c,
            get_remaining_compute_units_cost: cb.get_remaining_compute_units_cost,
            alt_bn128_g1_compress: cb.alt_bn128_g1_compress,
            alt_bn128_g1_decompress: cb.alt_bn128_g1_decompress,
            alt_bn128_g2_compress: cb.alt_bn128_g2_compress,
            alt_bn128_g2_decompress: cb.alt_bn128_g2_decompress,
        }
    }
}

impl From<ComputeBudgetState> for ComputeBudget {
    fn from(s: ComputeBudgetState) -> Self {
        ComputeBudget {
            compute_unit_limit: s.compute_unit_limit,
            log_64_units: s.log_64_units,
            create_program_address_units: s.create_program_address_units,
            invoke_units: s.invoke_units,
            max_instruction_stack_depth: s.max_instruction_stack_depth,
            max_instruction_trace_length: s.max_instruction_trace_length,
            sha256_base_cost: s.sha256_base_cost,
            sha256_byte_cost: s.sha256_byte_cost,
            sha256_max_slices: s.sha256_max_slices,
            max_call_depth: s.max_call_depth,
            stack_frame_size: s.stack_frame_size,
            log_pubkey_units: s.log_pubkey_units,
            cpi_bytes_per_unit: s.cpi_bytes_per_unit,
            sysvar_base_cost: s.sysvar_base_cost,
            secp256k1_recover_cost: s.secp256k1_recover_cost,
            syscall_base_cost: s.syscall_base_cost,
            curve25519_edwards_validate_point_cost: s.curve25519_edwards_validate_point_cost,
            curve25519_edwards_add_cost: s.curve25519_edwards_add_cost,
            curve25519_edwards_subtract_cost: s.curve25519_edwards_subtract_cost,
            curve25519_edwards_multiply_cost: s.curve25519_edwards_multiply_cost,
            curve25519_edwards_msm_base_cost: s.curve25519_edwards_msm_base_cost,
            curve25519_edwards_msm_incremental_cost: s.curve25519_edwards_msm_incremental_cost,
            curve25519_ristretto_validate_point_cost: s.curve25519_ristretto_validate_point_cost,
            curve25519_ristretto_add_cost: s.curve25519_ristretto_add_cost,
            curve25519_ristretto_subtract_cost: s.curve25519_ristretto_subtract_cost,
            curve25519_ristretto_multiply_cost: s.curve25519_ristretto_multiply_cost,
            curve25519_ristretto_msm_base_cost: s.curve25519_ristretto_msm_base_cost,
            curve25519_ristretto_msm_incremental_cost: s
                .curve25519_ristretto_msm_incremental_cost,
            heap_size: s.heap_size,
            heap_cost: s.heap_cost,
            mem_op_base_cost: s.mem_op_base_cost,
            alt_bn128_addition_cost: s.alt_bn128_addition_cost,
            alt_bn128_multiplication_cost: s.alt_bn128_multiplication_cost,
            alt_bn128_pairing_one_pair_cost_first: s.alt_bn128_pairing_one_pair_cost_first,
            alt_bn128_pairing_one_pair_cost_other: s.alt_bn128_pairing_one_pair_cost_other,
            big_modular_exponentiation_base_cost: s.big_modular_exponentiation_base_cost,
            big_modular_exponentiation_cost_divisor: s.big_modular_exponentiation_cost_divisor,
            poseidon_cost_coefficient_a: s.poseidon_cost_coefficient_a,
            poseidon_cost_coefficient_c: s.poseidon_cost_coefficient_c,
            get_remaining_compute_units_cost: s.get_remaining_compute_units_cost,
            alt_bn128_g1_compress: s.alt_bn128_g1_compress,
            alt_bn128_g1_decompress: s.alt_bn128_g1_decompress,
            alt_bn128_g2_compress: s.alt_bn128_g2_compress,
            alt_bn128_g2_decompress: s.alt_bn128_g2_decompress,
        }
    }
}

/// The complete serializable snapshot of LiteSVM state.
#[derive(Serialize, Deserialize)]
struct LiteSVMState {
    accounts: Vec<(Address, AccountSharedData)>,
    latest_blockhash: Hash,
    /// Stored as Vec<u8> because serde doesn't natively support [u8; 64].
    airdrop_kp: Vec<u8>,
    sigverify: bool,
    blockhash_check: bool,
    log_bytes_limit: Option<usize>,
    fee_structure: FeeStructureState,
    active_features: Vec<(Address, u64)>,
    compute_budget: Option<ComputeBudgetState>,
    history: Vec<(Signature, TransactionResultState)>,
    history_capacity: usize,
}

/// Extracts all serializable state from a `LiteSVM` instance.
fn extract_state(svm: &LiteSVM) -> LiteSVMState {
    let accounts: Vec<(Address, AccountSharedData)> = svm
        .accounts_db()
        .inner
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();

    let active_features: Vec<(Address, u64)> = svm
        .get_feature_set_ref()
        .active()
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();

    let history: Vec<(Signature, TransactionResultState)> = svm
        .transaction_history_entries()
        .iter()
        .map(|(sig, result)| (*sig, TransactionResultState::from(result)))
        .collect();

    LiteSVMState {
        accounts,
        latest_blockhash: svm.latest_blockhash(),
        airdrop_kp: svm.airdrop_keypair_bytes().to_vec(),
        sigverify: svm.get_sigverify(),
        blockhash_check: svm.get_blockhash_check(),
        log_bytes_limit: svm.get_log_bytes_limit(),
        fee_structure: FeeStructureState::from(svm.get_fee_structure()),
        active_features,
        compute_budget: svm.get_compute_budget().as_ref().map(ComputeBudgetState::from),
        history,
        history_capacity: svm.get_history_capacity(),
    }
}

/// Rebuilds a `LiteSVM` instance from a deserialized state snapshot.
fn restore_from_state(state: LiteSVMState) -> Result<LiteSVM, PersistenceError> {
    // Reconstruct the feature set from persisted active features.
    let mut feature_set = FeatureSet::default();
    for (feature_id, slot) in &state.active_features {
        feature_set.activate(feature_id, *slot);
    }

    // Build LiteSVM with feature set and builtins.
    // with_sysvars() initializes the sysvar cache (especially Clock)
    // because load_program() depends on Clock being available.
    // The saved sysvar accounts will overwrite these defaults during account loading.
    //
    // IMPORTANT: compute_budget must be set BEFORE with_builtins() because
    // set_builtins() reads self.compute_budget to create ProgramRuntimeEnvironments.
    let mut svm = LiteSVM::default().with_feature_set(feature_set);

    if let Some(cb_state) = state.compute_budget {
        svm = svm.with_compute_budget(cb_state.into());
    }

    let fee_structure: FeeStructure = state.fee_structure.into();

    svm = svm
        .with_builtins()
        .with_sysvars()
        .with_sigverify(state.sigverify)
        .with_blockhash_check(state.blockhash_check)
        .with_log_bytes_limit(state.log_bytes_limit)
        .with_transaction_history(state.history_capacity);

    svm.set_fee_structure(fee_structure);

    // Restore airdrop keypair and blockhash.
    let airdrop_kp: [u8; 64] = state
        .airdrop_kp
        .try_into()
        .map_err(|_| {
            PersistenceError::Serialize(
                bincode::ErrorKind::Custom(
                    "airdrop keypair must be exactly 64 bytes".into(),
                )
                .into(),
            )
        })?;
    svm.set_airdrop_keypair(airdrop_kp);
    svm.set_latest_blockhash(state.latest_blockhash);

    // === TWO-PASS ACCOUNT LOADING ===
    //
    // Pass 1: Insert all accounts WITHOUT loading programs into cache.
    //         This avoids MissingAccount errors when upgradeable programs
    //         are inserted before their ProgramData accounts.
    for (address, account_shared_data) in state.accounts {
        let account: Account = account_shared_data.into();
        svm.set_account_no_checks(address, account);
    }
    // Pass 2: Rebuild sysvar cache and program cache now that ALL accounts exist.
    svm.rebuild_caches()?;

    // Restore transaction history.
    let history_entries = state
        .history
        .into_iter()
        .map(|(sig, result_state)| (sig, TransactionResult::from(result_state)));
    svm.restore_transaction_history(history_entries);

    Ok(svm)
}

/// Stack size for serialization/deserialization threads (64 MB).
///
/// Large LiteSVM states with many accounts produce deeply nested bincode
/// serialization frames that overflow the default 8 MB thread stack.
const SERDE_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Run a closure on a dedicated thread with a large stack.
///
/// Bincode serialization of large Solana account sets recurses deeply,
/// overflowing the default 8 MB stack. This helper spawns a scoped thread
/// with [`SERDE_STACK_SIZE`] to handle the load safely.
fn with_large_stack<F, T>(f: F) -> Result<T, PersistenceError>
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("litesvm-persistence".into())
            .stack_size(SERDE_STACK_SIZE)
            .spawn_scoped(scope, f)
            .map_err(|e| PersistenceError::Io(io::Error::other(format!("failed to spawn serialization thread: {e}"))))?
            .join()
            .map_err(|_| PersistenceError::Io(io::Error::other("serialization thread panicked")))
    })
}

/// Saves the current `LiteSVM` state to a file using bincode serialization.
///
/// Serialization runs on a dedicated thread with a 64 MB stack to handle
/// deeply nested Solana types without stack overflow.
///
/// # Example
///
/// ```rust,no_run
/// use litesvm::LiteSVM;
/// use litesvm_persistence::{save_to_file, load_from_file};
///
/// let mut svm = LiteSVM::new();
/// // ... set up accounts, deploy programs, etc.
/// save_to_file(&svm, "snapshot.bin").unwrap();
///
/// // Later, restore the state:
/// let restored_svm = load_from_file("snapshot.bin").unwrap();
/// ```
pub fn save_to_file(svm: &LiteSVM, path: impl AsRef<Path>) -> Result<(), PersistenceError> {
    let state = extract_state(svm);
    let path = path.as_ref().to_path_buf();
    with_large_stack(move || -> Result<(), PersistenceError> {
        // Write to a temp file first, then atomically rename to avoid
        // leaving a corrupt file if the process crashes mid-write.
        let tmp_path = path.with_extension("tmp");

        // Serialize to memory, compute checksum, write payload + checksum.
        let bytes = bincode::serialize(&state)?;
        let checksum = crc32(&bytes);

        let file = fs::File::create(&tmp_path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&bytes)?;
        writer.write_all(&checksum.to_le_bytes())?;
        writer.flush()?;
        writer.into_inner().map_err(|e| e.into_error())?.sync_all()?;

        // Atomic rename.
        fs::rename(&tmp_path, &path)?;
        Ok(())
    })?
}

/// Loads a `LiteSVM` instance from a previously saved state file.
///
/// Deserialization runs on a dedicated thread with a 64 MB stack to handle
/// deeply nested Solana types without stack overflow.
///
/// # Example
///
/// ```rust,no_run
/// use litesvm_persistence::load_from_file;
///
/// let svm = load_from_file("snapshot.bin").unwrap();
/// // svm is ready to use - all accounts, programs, and config are restored
/// ```
pub fn load_from_file(path: impl AsRef<Path>) -> Result<LiteSVM, PersistenceError> {
    let mut bytes = Vec::new();
    BufReader::new(fs::File::open(path)?).read_to_end(&mut bytes)?;

    // Verify CRC32 checksum (last 4 bytes).
    if bytes.len() < 4 {
        return Err(PersistenceError::Serialize(
            bincode::ErrorKind::Custom("file too small to contain checksum".into()).into(),
        ));
    }
    let (payload, checksum_bytes) = bytes.split_at(bytes.len() - 4);
    verify_checksum(payload, checksum_bytes)?;

    let state: LiteSVMState = with_large_stack(move || bincode::deserialize(payload))??;
    restore_from_state(state)
}

/// Serializes the current `LiteSVM` state to bytes.
///
/// Useful when you want to manage storage yourself (e.g., store in a database
/// or send over a network). The returned bytes include a trailing CRC32
/// checksum for integrity verification.
pub fn to_bytes(svm: &LiteSVM) -> Result<Vec<u8>, PersistenceError> {
    let state = extract_state(svm);
    with_large_stack(move || -> Result<Vec<u8>, PersistenceError> {
        let mut bytes = bincode::serialize(&state)?;
        let checksum = crc32(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        Ok(bytes)
    })?
}

/// Deserializes a `LiteSVM` instance from bytes.
///
/// The bytes must have been produced by [`to_bytes`] and include
/// a trailing CRC32 checksum.
pub fn from_bytes(bytes: &[u8]) -> Result<LiteSVM, PersistenceError> {
    if bytes.len() < 4 {
        return Err(PersistenceError::Serialize(
            bincode::ErrorKind::Custom("data too small to contain checksum".into()).into(),
        ));
    }
    let (payload, checksum_bytes) = bytes.split_at(bytes.len() - 4);
    verify_checksum(payload, checksum_bytes)?;

    let state: LiteSVMState = with_large_stack(|| bincode::deserialize(payload))??;
    restore_from_state(state)
}

fn verify_checksum(payload: &[u8], checksum_bytes: &[u8]) -> Result<(), PersistenceError> {
    let expected = u32::from_le_bytes(checksum_bytes.try_into().unwrap());
    let actual = crc32(payload);
    if expected != actual {
        return Err(PersistenceError::ChecksumMismatch { expected, actual });
    }
    Ok(())
}
