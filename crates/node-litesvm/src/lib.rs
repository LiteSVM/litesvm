#![deny(clippy::all)]
use {
    bincode::deserialize,
    litesvm::{
        error::LiteSVMError,
        types::{
            FailedTransactionMetadata as FailedTransactionMetadataOriginal,
            SimulatedTransactionInfo as SimulatedTransactionInfoOriginal,
            TransactionMetadata as TransactionMetadataOriginal,
            TransactionResult as TransactionResultOriginal,
        },
        LiteSVM as LiteSVMOriginal,
    },
    napi::bindgen_prelude::*,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_sdk::{
        account::Account as AccountOriginal,
        feature_set::FeatureSet as FeatureSetOriginal,
        inner_instruction::InnerInstruction as InnerInstructionOriginal,
        instruction::CompiledInstruction as CompiledInstructionOriginal,
        pubkey::Pubkey,
        signature::Signature,
        transaction::{Transaction, VersionedTransaction},
        transaction_context::TransactionReturnData as TransactionReturnDataOriginal,
    },
};

#[macro_use]
extern crate napi_derive;

fn convert_pubkey(address: Uint8Array) -> Pubkey {
    Pubkey::try_from(address.as_ref()).unwrap()
}

fn to_js_error(e: LiteSVMError, msg: &str) -> Error {
    Error::new(Status::GenericFailure, format!("{msg}: {e}"))
}

#[derive(Debug, Clone)]
#[napi]
pub struct CompiledInstruction(CompiledInstructionOriginal);

#[napi]
impl CompiledInstruction {
    #[napi(constructor)]
    pub fn new(program_id_index: u8, accounts: Uint8Array, data: Uint8Array) -> Self {
        Self(CompiledInstructionOriginal {
            program_id_index,
            accounts: accounts.to_vec(),
            data: data.to_vec(),
        })
    }

    #[napi]
    pub fn program_id_index(&self) -> u8 {
        self.0.program_id_index
    }

    #[napi]
    pub fn accounts(&self) -> Uint8Array {
        Uint8Array::new(self.0.accounts.clone())
    }

    #[napi]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct InnerInstruction(InnerInstructionOriginal);

#[napi]
impl InnerInstruction {
    #[napi]
    pub fn instruction(&self) -> CompiledInstruction {
        CompiledInstruction(self.0.instruction.clone())
    }

    #[napi]
    pub fn stack_height(&self) -> u8 {
        self.0.stack_height
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct TransactionReturnData(TransactionReturnDataOriginal);

#[napi]
impl TransactionReturnData {
    #[napi]
    pub fn program_id(&self) -> Uint8Array {
        Uint8Array::with_data_copied(self.0.program_id)
    }

    #[napi]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct FeatureSet(FeatureSetOriginal);

#[napi]
impl FeatureSet {
    #[napi(factory, js_name = "default")]
    pub fn new_default() -> Self {
        Self(FeatureSetOriginal::default())
    }

    #[napi(factory)]
    pub fn all_enabled() -> Self {
        Self(FeatureSetOriginal::all_enabled())
    }

    #[napi]
    pub fn is_active(&self, feature_id: Uint8Array) -> bool {
        self.0.is_active(&convert_pubkey(feature_id))
    }

    #[napi]
    pub fn activated_slot(&self, feature_id: Uint8Array) -> Option<u64> {
        self.0.activated_slot(&convert_pubkey(feature_id))
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct TransactionMetadata(TransactionMetadataOriginal);

#[napi]
impl TransactionMetadata {
    #[napi]
    pub fn signature(&self) -> Uint8Array {
        Uint8Array::with_data_copied(self.0.signature)
    }

    #[napi]
    pub fn logs(&self) -> Vec<String> {
        self.0.logs.clone()
    }

    #[napi]
    pub fn inner_instructions(&self) -> Vec<Vec<InnerInstruction>> {
        self.0
            .inner_instructions
            .clone()
            .into_iter()
            .map(|outer| outer.into_iter().map(InnerInstruction).collect())
            .collect()
    }

    #[napi]
    pub fn compute_units_consumed(&self) -> u64 {
        self.0.compute_units_consumed
    }

    #[napi]
    pub fn return_data(&self) -> TransactionReturnData {
        TransactionReturnData(self.0.return_data.clone())
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct FailedTransactionMetadata(FailedTransactionMetadataOriginal);

#[napi]
impl FailedTransactionMetadata {
    #[napi]
    pub fn err(&self) -> String {
        self.0.err.to_string()
    }

    #[napi]
    pub fn meta(&self) -> TransactionMetadata {
        TransactionMetadata(self.0.meta.clone())
    }
}

#[derive(Clone)]
#[napi]
pub struct AddressAndAccount {
    pub address: Uint8Array,
    account: Account,
}

#[napi]
impl AddressAndAccount {
    #[napi]
    pub fn account(&self) -> Account {
        self.account.clone()
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct SimulatedTransactionInfo(SimulatedTransactionInfoOriginal);

#[napi]
impl SimulatedTransactionInfo {
    #[napi]
    pub fn meta(&self) -> TransactionMetadata {
        TransactionMetadata(self.0.meta.clone())
    }

    #[napi]
    pub fn post_accounts(&self) -> Vec<AddressAndAccount> {
        self.0
            .post_accounts
            .clone()
            .into_iter()
            .map(|x| AddressAndAccount {
                address: Uint8Array::with_data_copied(x.0),
                account: Account(AccountOriginal::from(x.1)),
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
#[napi]
pub struct Account(AccountOriginal);

impl AsRef<AccountOriginal> for Account {
    fn as_ref(&self) -> &AccountOriginal {
        &self.0
    }
}

#[napi]
impl Account {
    #[napi(constructor)]
    pub fn new(
        lamports: BigInt,
        data: Uint8Array,
        owner: Uint8Array,
        executable: bool,
        rent_epoch: BigInt,
    ) -> Self {
        Self(AccountOriginal {
            lamports: lamports.get_u64().1,
            data: data.to_vec(),
            owner: Pubkey::try_from(owner.as_ref()).unwrap(),
            executable,
            rent_epoch: rent_epoch.get_u64().1,
        })
    }

    #[napi]
    pub fn lamports(&self) -> u64 {
        self.0.lamports
    }

    #[napi]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }

    #[napi]
    pub fn owner(&self) -> Uint8Array {
        Uint8Array::new(self.0.owner.to_bytes().to_vec())
    }

    #[napi]
    pub fn executable(&self) -> bool {
        self.0.executable
    }

    #[napi]
    pub fn rent_epoch(&self) -> u64 {
        self.0.rent_epoch
    }
}

pub type TransactionResult = Either<TransactionMetadata, FailedTransactionMetadata>;
pub type SimulateResult = Either<SimulatedTransactionInfo, FailedTransactionMetadata>;

fn convert_transaction_result(inner: TransactionResultOriginal) -> TransactionResult {
    match inner {
        Ok(x) => TransactionResult::A(TransactionMetadata(x)),
        Err(e) => TransactionResult::B(FailedTransactionMetadata(e)),
    }
}

fn convert_sim_result(
    inner: std::result::Result<SimulatedTransactionInfoOriginal, FailedTransactionMetadataOriginal>,
) -> SimulateResult {
    match inner {
        Ok(x) => SimulateResult::A(SimulatedTransactionInfo(x)),
        Err(e) => SimulateResult::B(FailedTransactionMetadata(e)),
    }
}

#[napi]
pub struct LiteSVM(LiteSVMOriginal);

#[napi]
impl LiteSVM {
    /// Creates the basic test environment.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self(LiteSVMOriginal::new())
    }

    #[napi]
    /// Sets the compute budget.
    // napi-rs doesn't support passing custom structs as params,
    // so we have this ugly thing
    pub fn set_compute_budget(
        &mut self,
        compute_unit_limit: BigInt,
        log_64_units: BigInt,
        create_program_address_units: BigInt,
        invoke_units: BigInt,
        max_instruction_stack_depth: BigInt,
        max_instruction_trace_length: BigInt,
        sha256_base_cost: BigInt,
        sha256_byte_cost: BigInt,
        sha256_max_slices: BigInt,
        max_call_depth: BigInt,
        stack_frame_size: BigInt,
        log_pubkey_units: BigInt,
        max_cpi_instruction_size: BigInt,
        cpi_bytes_per_unit: BigInt,
        sysvar_base_cost: BigInt,
        secp256k1_recover_cost: BigInt,
        syscall_base_cost: BigInt,
        curve25519_edwards_validate_point_cost: BigInt,
        curve25519_edwards_add_cost: BigInt,
        curve25519_edwards_subtract_cost: BigInt,
        curve25519_edwards_multiply_cost: BigInt,
        curve25519_edwards_msm_base_cost: BigInt,
        curve25519_edwards_msm_incremental_cost: BigInt,
        curve25519_ristretto_validate_point_cost: BigInt,
        curve25519_ristretto_add_cost: BigInt,
        curve25519_ristretto_subtract_cost: BigInt,
        curve25519_ristretto_multiply_cost: BigInt,
        curve25519_ristretto_msm_base_cost: BigInt,
        curve25519_ristretto_msm_incremental_cost: BigInt,
        heap_size: u32,
        heap_cost: BigInt,
        mem_op_base_cost: BigInt,
        alt_bn128_addition_cost: BigInt,
        alt_bn128_multiplication_cost: BigInt,
        alt_bn128_pairing_one_pair_cost_first: BigInt,
        alt_bn128_pairing_one_pair_cost_other: BigInt,
        big_modular_exponentiation_base_cost: BigInt,
        big_modular_exponentiation_cost_divisor: BigInt,
        poseidon_cost_coefficient_a: BigInt,
        poseidon_cost_coefficient_c: BigInt,
        get_remaining_compute_units_cost: BigInt,
        alt_bn128_g1_compress: BigInt,
        alt_bn128_g1_decompress: BigInt,
        alt_bn128_g2_compress: BigInt,
        alt_bn128_g2_decompress: BigInt,
    ) {
        let inner = ComputeBudget {
            compute_unit_limit: compute_unit_limit.get_u64().1,
            log_64_units: log_64_units.get_u64().1,
            create_program_address_units: create_program_address_units.get_u64().1,
            invoke_units: invoke_units.get_u64().1,
            max_instruction_stack_depth: usize::try_from(max_instruction_stack_depth.get_u64().1)
                .unwrap(),
            max_instruction_trace_length: usize::try_from(max_instruction_trace_length.get_u64().1)
                .unwrap(),
            sha256_base_cost: sha256_base_cost.get_u64().1,
            sha256_byte_cost: sha256_byte_cost.get_u64().1,
            sha256_max_slices: sha256_max_slices.get_u64().1,
            max_call_depth: usize::try_from(max_call_depth.get_u64().1).unwrap(),
            stack_frame_size: usize::try_from(stack_frame_size.get_u64().1).unwrap(),
            log_pubkey_units: log_pubkey_units.get_u64().1,
            max_cpi_instruction_size: usize::try_from(max_cpi_instruction_size.get_u64().1)
                .unwrap(),
            cpi_bytes_per_unit: cpi_bytes_per_unit.get_u64().1,
            sysvar_base_cost: sysvar_base_cost.get_u64().1,
            secp256k1_recover_cost: secp256k1_recover_cost.get_u64().1,
            syscall_base_cost: syscall_base_cost.get_u64().1,
            curve25519_edwards_validate_point_cost: curve25519_edwards_validate_point_cost
                .get_u64()
                .1,
            curve25519_edwards_add_cost: curve25519_edwards_add_cost.get_u64().1,
            curve25519_edwards_subtract_cost: curve25519_edwards_subtract_cost.get_u64().1,
            curve25519_edwards_multiply_cost: curve25519_edwards_multiply_cost.get_u64().1,
            curve25519_edwards_msm_base_cost: curve25519_edwards_msm_base_cost.get_u64().1,
            curve25519_edwards_msm_incremental_cost: curve25519_edwards_msm_incremental_cost
                .get_u64()
                .1,
            curve25519_ristretto_validate_point_cost: curve25519_ristretto_validate_point_cost
                .get_u64()
                .1,
            curve25519_ristretto_add_cost: curve25519_ristretto_add_cost.get_u64().1,
            curve25519_ristretto_subtract_cost: curve25519_ristretto_subtract_cost.get_u64().1,
            curve25519_ristretto_multiply_cost: curve25519_ristretto_multiply_cost.get_u64().1,
            curve25519_ristretto_msm_base_cost: curve25519_ristretto_msm_base_cost.get_u64().1,
            curve25519_ristretto_msm_incremental_cost: curve25519_ristretto_msm_incremental_cost
                .get_u64()
                .1,
            heap_size,
            heap_cost: heap_cost.get_u64().1,
            mem_op_base_cost: mem_op_base_cost.get_u64().1,
            alt_bn128_addition_cost: alt_bn128_addition_cost.get_u64().1,
            alt_bn128_multiplication_cost: alt_bn128_multiplication_cost.get_u64().1,
            alt_bn128_pairing_one_pair_cost_first: alt_bn128_pairing_one_pair_cost_first
                .get_u64()
                .1,
            alt_bn128_pairing_one_pair_cost_other: alt_bn128_pairing_one_pair_cost_other
                .get_u64()
                .1,
            big_modular_exponentiation_base_cost: big_modular_exponentiation_base_cost.get_u64().1,
            big_modular_exponentiation_cost_divisor: big_modular_exponentiation_cost_divisor
                .get_u64()
                .1,
            poseidon_cost_coefficient_a: poseidon_cost_coefficient_a.get_u64().1,
            poseidon_cost_coefficient_c: poseidon_cost_coefficient_c.get_u64().1,
            get_remaining_compute_units_cost: get_remaining_compute_units_cost.get_u64().1,
            alt_bn128_g1_compress: alt_bn128_g1_compress.get_u64().1,
            alt_bn128_g1_decompress: alt_bn128_g1_decompress.get_u64().1,
            alt_bn128_g2_compress: alt_bn128_g2_compress.get_u64().1,
            alt_bn128_g2_decompress: alt_bn128_g2_decompress.get_u64().1,
        };
        self.0.set_compute_budget(inner);
    }
    // /// Enables or disables sigverify
    // pub fn with_sigverify(self, sigverify: bool) -> Self {
    //     Self(self.0.with_sigverify(sigverify))
    // }
    // /// Enables or disables the blockhash check
    // pub fn with_blockhash_check(self, check: bool) -> Self {
    //     Self(self.0.with_blockhash_check(check))
    // }
    // /// Includes the default sysvars
    // pub fn with_sysvars(self) -> Self {
    //     Self(self.0.with_sysvars())
    // }

    // /// Changes the default builtins
    // pub fn with_builtins(self, feature_set: Option<FeatureSet>) -> Self {
    //     Self(self.0.with_builtins(feature_set.map(|x| x.into())))
    // }

    // /// Changes the initial lamports in LiteSVM's airdrop account
    // pub fn with_lamports(self, lamports: u64) -> Self {
    //     Self(self.0.with_lamports(lamports))
    // }

    // /// Includes the standard SPL programs
    // pub fn with_spl_programs(self) -> Self {
    //     Self(self.0.with_spl_programs())
    // }

    /// Changes the capacity of the transaction history.
    /// Set this to 0 to disable transaction history and allow duplicate transactions.
    // pub fn with_transaction_history(self, capacity: usize) -> Self {
    //     Self(self.0.with_transaction_history(capacity))
    // }

    // pub fn with_log_bytes_limit(self, limit: Option<usize>) -> Self {
    //     Self(self.0.with_log_bytes_limit(limit))
    // }

    // pub fn with_precompiles(self, feature_set: Option<FeatureSet>) -> Self {
    //     Self(self.0.with_precompiles(feature_set.map(|x| x.into())))
    // }

    #[napi]
    /// Returns minimum balance required to make an account with specified data length rent exempt.
    pub fn minimum_balance_for_rent_exemption(&self, data_len: BigInt) -> u64 {
        self.0
            .minimum_balance_for_rent_exemption(data_len.get_u64().1 as usize)
    }

    #[napi]
    /// Returns all information associated with the account of the provided pubkey.
    pub fn get_account(&self, pubkey: Uint8Array) -> Option<Account> {
        self.0.get_account(&convert_pubkey(pubkey)).map(Account)
    }

    #[napi]
    /// Sets all information associated with the account of the provided pubkey.
    pub fn set_account(&mut self, pubkey: Uint8Array, data: &Account) -> Result<()> {
        self.0
            .set_account(convert_pubkey(pubkey), data.0.clone())
            .map_err(|e| to_js_error(e, "Failed to set account"))
    }

    #[napi]
    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&self, pubkey: Uint8Array) -> Option<u64> {
        self.0.get_balance(&convert_pubkey(pubkey))
    }

    #[napi]
    /// Gets the latest blockhash.
    pub fn latest_blockhash(&self) -> String {
        self.0.latest_blockhash().to_string()
    }

    #[napi]
    /// Gets a transaction from the transaction history.
    pub fn get_transaction(&self, signature: Uint8Array) -> Option<TransactionResult> {
        self.0
            .get_transaction(&Signature::try_from(signature.as_ref()).unwrap())
            .map(|x| convert_transaction_result(x.clone()))
    }

    #[napi]
    /// Airdrops the account with the lamports specified.
    pub fn airdrop(&mut self, pubkey: Uint8Array, lamports: BigInt) -> TransactionResult {
        convert_transaction_result(
            self.0
                .airdrop(&convert_pubkey(pubkey), lamports.get_u64().1),
        )
    }

    #[napi]
    /// Adds am SBF program to the test environment from the file specified.
    pub fn add_program_from_file(&mut self, program_id: Uint8Array, path: String) -> Result<()> {
        self.0
            .add_program_from_file(convert_pubkey(program_id), path)
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to add program: {e}"),
                )
            })
    }

    #[napi]
    /// Adds am SBF program to the test environment.
    pub fn add_program(&mut self, program_id: Uint8Array, program_bytes: &[u8]) {
        self.0
            .add_program(convert_pubkey(program_id), program_bytes)
    }

    #[napi(ts_return_type = "TransactionMetadata | FailedTransactionMetadata")]
    pub fn send_legacy_transaction(&mut self, tx_bytes: Uint8Array) -> TransactionResult {
        let tx: Transaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.send_transaction(tx);
        convert_transaction_result(res)
    }

    #[napi(ts_return_type = "TransactionMetadata | FailedTransactionMetadata")]
    pub fn send_versioned_transaction(&mut self, tx_bytes: Uint8Array) -> TransactionResult {
        let tx: VersionedTransaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.send_transaction(tx);
        convert_transaction_result(res)
    }

    #[napi(ts_return_type = "SimulatedTransactionInfo | FailedTransactionMetadata")]
    pub fn simulate_legacy_transaction(&mut self, tx_bytes: Uint8Array) -> SimulateResult {
        let tx: Transaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.simulate_transaction(tx);
        convert_sim_result(res)
    }

    #[napi(ts_return_type = "SimulatedTransactionInfo | FailedTransactionMetadata")]
    pub fn simulate_versioned_transaction(&mut self, tx_bytes: Uint8Array) -> SimulateResult {
        let tx: VersionedTransaction = deserialize(&tx_bytes).unwrap();
        let res = self.0.simulate_transaction(tx);
        convert_sim_result(res)
    }

    #[napi]
    /// Expires the current blockhash
    pub fn expire_blockhash(&mut self) {
        self.0.expire_blockhash()
    }

    #[napi]
    /// Warps the clock to the specified slot
    pub fn warp_to_slot(&mut self, slot: BigInt) {
        self.0.warp_to_slot(slot.get_u64().1)
    }
}
