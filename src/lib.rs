#![allow(clippy::result_large_err)]
use log::error;

use crate::error::LiteSVMError;
use itertools::Itertools;
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_loader_v4_program::create_program_runtime_environment_v2;
#[allow(deprecated)]
use solana_program::sysvar::{fees::Fees, recent_blockhashes::RecentBlockhashes};
use solana_program_runtime::{
    compute_budget::ComputeBudget,
    compute_budget_processor::{process_compute_budget_instructions, ComputeBudgetLimits},
    invoke_context::BuiltinFunctionWithContext,
    loaded_programs::{LoadProgramMetrics, LoadedProgram, LoadedProgramsForTxBatch},
    log_collector::LogCollector,
    message_processor::MessageProcessor,
    timings::ExecuteTimings,
};
#[allow(deprecated)]
use solana_sdk::sysvar::recent_blockhashes::IterItem;
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    bpf_loader,
    clock::Clock,
    epoch_rewards::EpochRewards,
    epoch_schedule::EpochSchedule,
    feature_set::{include_loaded_accounts_data_size_in_fee_calculation, FeatureSet},
    fee::FeeStructure,
    hash::Hash,
    message::{Message, SanitizedMessage, VersionedMessage},
    native_loader,
    native_token::LAMPORTS_PER_SOL,
    nonce::{state::DurableNonce, NONCED_TX_MARKER_IX_INDEX},
    nonce_account,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signature},
    signer::Signer,
    signers::Signers,
    slot_hashes::SlotHashes,
    slot_history::SlotHistory,
    stake_history::StakeHistory,
    system_instruction, system_program,
    sysvar::{last_restart_slot::LastRestartSlot, Sysvar, SysvarId},
    transaction::{MessageHash, SanitizedTransaction, TransactionError, VersionedTransaction},
    transaction_context::{ExecutionRecord, IndexOfAccount, TransactionContext},
};
use solana_system_program::{get_system_account_kind, SystemAccountKind};
use std::{cell::RefCell, path::Path, rc::Rc, sync::Arc};
use utils::construct_instructions_account;

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    history::TransactionHistory,
    spl::load_spl_programs,
    types::{ExecutionResult, FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    utils::{
        create_blockhash,
        loader::{deploy_upgradeable_program, set_upgrade_authority},
        rent::RentState,
    },
};

pub mod error;
pub mod types;

mod accounts_db;
mod builtin;
mod history;
mod spl;
mod utils;

// The test code doesn't actually get run because it's not
// what doctest expects but at least it
// compiles it so we'll see if there's a compile-time error.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

pub struct LiteSVM {
    accounts: AccountsDb,
    airdrop_kp: Keypair,
    feature_set: Arc<FeatureSet>,
    latest_blockhash: Hash,
    log_collector: Rc<RefCell<LogCollector>>,
    history: TransactionHistory,
    compute_budget: Option<ComputeBudget>,
    sigverify: bool,
    blockhash_check: bool,
    fee_structure: FeeStructure,
}

impl Default for LiteSVM {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            airdrop_kp: Keypair::new(),
            feature_set: Default::default(),
            latest_blockhash: create_blockhash(b"genesis"),
            log_collector: Default::default(),
            history: TransactionHistory::new(),
            compute_budget: None,
            sigverify: false,
            blockhash_check: false,
            fee_structure: FeeStructure::default(),
        }
    }
}

impl LiteSVM {
    pub fn new() -> Self {
        LiteSVM::default()
            .with_builtins()
            .with_lamports(1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL))
            .with_sysvars()
            .with_spl_programs()
            .with_sigverify(true)
            .with_blockhash_check(true)
    }

    pub fn with_compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        self.compute_budget = Some(compute_budget);
        self
    }

    pub fn with_sigverify(mut self, sigverify: bool) -> Self {
        self.sigverify = sigverify;
        self
    }

    pub fn with_blockhash_check(mut self, check: bool) -> Self {
        self.blockhash_check = check;
        self
    }

    pub fn with_sysvars(mut self) -> Self {
        self.set_sysvar(&Clock::default());
        self.set_sysvar(&EpochRewards::default());
        self.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        let fees = Fees::default();
        self.set_sysvar(&fees);
        self.set_sysvar(&LastRestartSlot::default());
        let latest_blockhash = self.latest_blockhash;
        #[allow(deprecated)]
        self.set_sysvar(&RecentBlockhashes::from_iter([IterItem(
            0,
            &latest_blockhash,
            fees.fee_calculator.lamports_per_signature,
        )]));
        self.set_sysvar(&Rent::default());
        self.set_sysvar(&SlotHashes::new(&[(
            self.accounts.sysvar_cache.get_clock().unwrap().slot,
            latest_blockhash,
        )]));
        self.set_sysvar(&SlotHistory::default());
        self.set_sysvar(&StakeHistory::default());
        self
    }

    pub fn with_builtins(mut self) -> Self {
        let mut feature_set = FeatureSet::all_enabled();

        BUILTINS.iter().for_each(|builtint| {
            let loaded_program =
                LoadedProgram::new_builtin(0, builtint.name.len(), builtint.entrypoint);
            self.accounts
                .programs_cache
                .replenish(builtint.program_id, Arc::new(loaded_program));
            self.accounts.add_builtin_account(
                builtint.program_id,
                native_loader::create_loadable_account_for_test(builtint.name),
            );

            if let Some(feature_id) = builtint.feature_id {
                feature_set.activate(&feature_id, 0);
            }
        });

        let program_runtime_v1 = create_program_runtime_environment_v1(
            &feature_set,
            &ComputeBudget::default(),
            false,
            true,
        )
        .unwrap();

        let program_runtime_v2 =
            create_program_runtime_environment_v2(&ComputeBudget::default(), true);

        self.accounts.programs_cache.environments.program_runtime_v1 = Arc::new(program_runtime_v1);
        self.accounts.programs_cache.environments.program_runtime_v2 = Arc::new(program_runtime_v2);
        self.feature_set = Arc::new(feature_set);
        self
    }

    pub fn with_lamports(mut self, lamports: u64) -> Self {
        self.accounts.add_account_no_checks(
            self.airdrop_kp.pubkey(),
            AccountSharedData::new(lamports, 0, &system_program::id()),
        );
        self
    }

    pub fn with_spl_programs(mut self) -> Self {
        load_spl_programs(&mut self);
        self
    }

    pub fn with_transaction_history(mut self, capacity: usize) -> Self {
        self.history.set_capacity(capacity);
        self
    }

    pub fn minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
        1.max(
            self.accounts
                .sysvar_cache
                .get_rent()
                .unwrap_or_default()
                .minimum_balance(data_len),
        )
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        self.accounts.get_account(pubkey).map(Into::into)
    }

    pub fn set_account(&mut self, pubkey: Pubkey, data: Account) -> Result<(), LiteSVMError> {
        self.accounts.add_account(pubkey, data.into())
    }

    pub fn get_balance(&self, pubkey: &Pubkey) -> Option<u64> {
        self.accounts.get_account(pubkey).map(|x| x.lamports())
    }

    pub fn latest_blockhash(&self) -> Hash {
        self.latest_blockhash
    }

    pub fn set_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        let account = AccountSharedData::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();
        self.accounts.add_account(T::id(), account).unwrap();
    }

    pub fn get_sysvar<T>(&self) -> T
    where
        T: Sysvar + SysvarId,
    {
        bincode::deserialize(self.accounts.get_account(&T::id()).unwrap().data()).unwrap()
    }

    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionResult> {
        self.history.get_transaction(signature)
    }

    pub fn airdrop(&mut self, pubkey: &Pubkey, lamports: u64) -> TransactionResult {
        let payer = &self.airdrop_kp;
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(Message::new_with_blockhash(
                &[system_instruction::transfer(
                    &payer.pubkey(),
                    pubkey,
                    lamports,
                )],
                Some(&payer.pubkey()),
                &self.latest_blockhash,
            )),
            &[payer],
        )
        .unwrap();

        self.send_transaction(tx)
    }

    pub fn add_builtin(&mut self, program_id: Pubkey, entrypoint: BuiltinFunctionWithContext) {
        let builtin = LoadedProgram::new_builtin(
            self.accounts
                .sysvar_cache
                .get_clock()
                .unwrap_or_default()
                .slot,
            1,
            entrypoint,
        );

        self.accounts
            .programs_cache
            .replenish(program_id, Arc::new(builtin));
        self.accounts
            .add_account(program_id, AccountSharedData::new(0, 1, &bpf_loader::id()))
            .unwrap();
    }

    pub fn add_program_from_file(
        &mut self,
        program_id: Pubkey,
        path: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        let bytes = std::fs::read(path)?;
        self.add_program(program_id, &bytes);
        Ok(())
    }

    pub fn add_program(&mut self, program_id: Pubkey, program_bytes: &[u8]) {
        let program_len = program_bytes.len();
        let lamports = self.minimum_balance_for_rent_exemption(program_len);
        let mut account = AccountSharedData::new(lamports, program_len, &bpf_loader::id());
        account.set_executable(true);
        account.set_data_from_slice(program_bytes);
        let current_slot = self
            .accounts
            .sysvar_cache
            .get_clock()
            .unwrap_or_default()
            .slot;
        let mut loaded_program = solana_bpf_loader_program::load_program_from_bytes(
            Some(self.log_collector.clone()),
            &mut LoadProgramMetrics::default(),
            account.data(),
            account.owner(),
            account.data().len(),
            current_slot,
            self.accounts
                .programs_cache
                .environments
                .program_runtime_v1
                .clone(),
            false,
        )
        .unwrap_or_default();
        loaded_program.effective_slot = current_slot;
        self.accounts.add_account(program_id, account).unwrap();
        self.accounts
            .programs_cache
            .replenish(program_id, Arc::new(loaded_program));
    }

    pub fn deploy_upgradeable_program(
        &mut self,
        payer_kp: &Keypair,
        program_kp: &Keypair,
        program_bytes: &[u8],
    ) -> Result<(), FailedTransactionMetadata> {
        deploy_upgradeable_program(self, payer_kp, program_kp, program_bytes)
    }

    pub fn set_upgrade_authority(
        &mut self,
        from_keypair: &Keypair,
        program_pubkey: &Pubkey,
        current_authority_keypair: &Keypair,
        new_authority_pubkey: Option<&Pubkey>,
    ) -> Result<(), FailedTransactionMetadata> {
        set_upgrade_authority(
            self,
            from_keypair,
            program_pubkey,
            current_authority_keypair,
            new_authority_pubkey,
        )
    }

    fn create_transaction_context(
        &self,
        compute_budget: ComputeBudget,
        accounts: Vec<(Pubkey, AccountSharedData)>,
    ) -> TransactionContext {
        TransactionContext::new(
            accounts,
            self.get_sysvar(),
            compute_budget.max_invoke_stack_height,
            compute_budget.max_instruction_trace_length,
        )
    }

    fn sanitize_transaction_no_verify(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        SanitizedTransaction::try_create(tx, MessageHash::Compute, Some(false), &self.accounts)
    }

    fn sanitize_transaction(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        let tx = self.sanitize_transaction_no_verify(tx)?;

        tx.verify()?;
        tx.verify_precompiles(&self.feature_set)?;

        Ok(tx)
    }

    fn process_transaction(
        &self,
        tx: &SanitizedTransaction,
        compute_budget_limits: ComputeBudgetLimits,
    ) -> (
        Result<(), TransactionError>,
        u64,
        Option<TransactionContext>,
        u64,
        Option<Pubkey>,
    ) {
        let compute_budget = self.compute_budget.unwrap_or_else(|| ComputeBudget {
            compute_unit_limit: u64::from(compute_budget_limits.compute_unit_limit),
            heap_size: compute_budget_limits.updated_heap_bytes,
            ..ComputeBudget::default()
        });
        let blockhash = tx.message().recent_blockhash();
        //reload program cache
        let mut programs_modified_by_tx = LoadedProgramsForTxBatch::new(
            self.accounts.sysvar_cache.get_clock().unwrap().slot,
            self.accounts.programs_cache.environments.clone(),
            None,
            0,
        );
        let mut accumulated_consume_units = 0;
        let message = tx.message();
        let account_keys = message.account_keys();
        let instruction_accounts = message
            .instructions()
            .iter()
            .flat_map(|instruction| &instruction.accounts)
            .unique()
            .collect::<Vec<&u8>>();
        let fee = self.fee_structure.calculate_fee(
            message,
            self.fee_structure.lamports_per_signature,
            &compute_budget_limits.into(),
            self.feature_set
                .is_active(&include_loaded_accounts_data_size_in_fee_calculation::id()),
        );
        let mut validated_fee_payer = false;
        let mut payer_key = None;
        let maybe_accounts = account_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let mut account_found = true;
                let account = if solana_sdk::sysvar::instructions::check_id(key) {
                    construct_instructions_account(message)
                } else {
                    let instruction_account = u8::try_from(i)
                        .map(|i| instruction_accounts.contains(&&i))
                        .unwrap_or(false);
                    let mut account = if !instruction_account
                        && !message.is_writable(i)
                        && self.accounts.programs_cache.find(key).is_some()
                    {
                        // Optimization to skip loading of accounts which are only used as
                        // programs in top-level instructions and not passed as instruction accounts.
                        self.accounts.get_account(key).unwrap()
                    } else {
                        self.accounts.get_account(key).unwrap_or_else(|| {
                            account_found = false;
                            let mut default_account = AccountSharedData::default();
                            default_account.set_rent_epoch(0);
                            default_account
                        })
                    };
                    if !validated_fee_payer && message.is_non_loader_key(i) {
                        validate_fee_payer(
                            key,
                            &mut account,
                            i as IndexOfAccount,
                            &self.accounts.sysvar_cache.get_rent().unwrap(),
                            fee,
                        )?;
                        validated_fee_payer = true;
                        payer_key = Some(*key);
                    }
                    account
                };

                Ok((*key, account))
            })
            .collect::<solana_sdk::transaction::Result<Vec<_>>>();
        let mut accounts = match maybe_accounts {
            Ok(accs) => accs,
            Err(e) => {
                return (Err(e), accumulated_consume_units, None, fee, payer_key);
            }
        };
        if !validated_fee_payer {
            error!("Failed to validate fee payer");
            return (
                Err(TransactionError::AccountNotFound),
                accumulated_consume_units,
                None,
                fee,
                payer_key,
            );
        }
        let builtins_start_index = accounts.len();
        let maybe_program_indices = tx
            .message()
            .instructions()
            .iter()
            .map(|c| {
                let mut account_indices: Vec<u16> = Vec::with_capacity(2);
                let program_index = c.program_id_index as usize;
                // This may never error, because the transaction is sanitized
                let (program_id, program_account) = accounts.get(program_index).unwrap();
                if native_loader::check_id(program_id) {
                    return Ok(account_indices);
                }
                if !program_account.executable() {
                    error!("Program account {program_id} is not executable.");
                    return Err(TransactionError::InvalidProgramForExecution);
                }
                account_indices.insert(0, program_index as IndexOfAccount);

                let owner_id = program_account.owner();
                if native_loader::check_id(owner_id) {
                    return Ok(account_indices);
                }
                account_indices.insert(
                    0,
                    if let Some(owner_index) = accounts
                        .get(builtins_start_index..)
                        .unwrap()
                        .iter()
                        .position(|(key, _)| key == owner_id)
                    {
                        builtins_start_index.saturating_add(owner_index) as u16
                    } else {
                        let owner_index = accounts.len() as u16;
                        let owner_account = self.get_account(owner_id).unwrap();
                        if !native_loader::check_id(owner_account.owner()) {
                            error!("Owner account {owner_id} is not owned by the native loader program.");
                            return Err(TransactionError::InvalidProgramForExecution)
                        }
                        if !owner_account.executable {
                            error!("Owner account {owner_id} is not executable");
                            return Err(TransactionError::InvalidProgramForExecution)
                        }
                        accounts.push((*owner_id, owner_account.into()));
                        owner_index
                    },
                );
                Ok(account_indices)
            })
            .collect::<Result<Vec<Vec<u16>>, TransactionError>>();
        match maybe_program_indices {
            Ok(program_indices) => {
                let mut context = self.create_transaction_context(compute_budget, accounts);
                let mut tx_result = MessageProcessor::process_message(
                    tx.message(),
                    &program_indices,
                    &mut context,
                    Some(self.log_collector.clone()),
                    &self.accounts.programs_cache,
                    &mut programs_modified_by_tx,
                    self.feature_set.clone(),
                    compute_budget,
                    &mut ExecuteTimings::default(),
                    &self.accounts.sysvar_cache,
                    *blockhash,
                    0,
                    &mut accumulated_consume_units,
                )
                .map(|_| ());

                if let Err(err) = self.check_accounts_rent(tx, &context) {
                    tx_result = Err(err);
                };

                (
                    tx_result,
                    accumulated_consume_units,
                    Some(context),
                    fee,
                    payer_key,
                )
            }
            Err(e) => (Err(e), accumulated_consume_units, None, fee, payer_key),
        }
    }

    fn check_accounts_rent(
        &self,
        tx: &SanitizedTransaction,
        context: &TransactionContext,
    ) -> Result<(), TransactionError> {
        for index in 0..tx.message().account_keys().len() {
            if tx.message().is_writable(index) {
                let account = context
                    .get_account_at_index(index as IndexOfAccount)
                    .map_err(|err| TransactionError::InstructionError(index as u8, err))?
                    .borrow();
                let pubkey = context
                    .get_key_of_account_at_index(index as IndexOfAccount)
                    .map_err(|err| TransactionError::InstructionError(index as u8, err))?;
                let rent = self.accounts.sysvar_cache.get_rent().unwrap_or_default();

                if !account.data().is_empty() {
                    let post_rent_state = RentState::from_account(&account, &rent);
                    let pre_rent_state = RentState::from_account(
                        &self.accounts.get_account(pubkey).unwrap_or_default(),
                        &rent,
                    );

                    if !post_rent_state.transition_allowed_from(&pre_rent_state) {
                        return Err(TransactionError::InsufficientFundsForRent {
                            account_index: index as u8,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_transaction_no_verify(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        let sanitized_tx = match self.sanitize_transaction_no_verify(tx) {
            Ok(s_tx) => s_tx,
            Err(err) => {
                return ExecutionResult {
                    tx_result: Err(err),
                    ..Default::default()
                };
            }
        };
        self.execute_sanitized_transaction(sanitized_tx)
    }

    fn execute_transaction(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        let sanitized_tx = match self.sanitize_transaction(tx) {
            Ok(s_tx) => s_tx,
            Err(err) => {
                return ExecutionResult {
                    tx_result: Err(err),
                    ..Default::default()
                }
            }
        };
        self.execute_sanitized_transaction(sanitized_tx)
    }

    fn execute_sanitized_transaction(
        &mut self,
        sanitized_tx: SanitizedTransaction,
    ) -> ExecutionResult {
        if self.blockhash_check {
            if let Err(e) = self.check_transaction_age(&sanitized_tx) {
                return ExecutionResult {
                    tx_result: Err(e),
                    ..Default::default()
                };
            }
        }
        let instructions = sanitized_tx.message().program_instructions_iter();
        let compute_budget_limits = match process_compute_budget_instructions(instructions) {
            Ok(x) => x,
            Err(e) => {
                return ExecutionResult {
                    tx_result: Err(e),
                    ..Default::default()
                };
            }
        };
        if self.history.check_transaction(sanitized_tx.signature()) {
            return ExecutionResult {
                tx_result: Err(TransactionError::AlreadyProcessed),
                ..Default::default()
            };
        }

        let (result, compute_units_consumed, context, fee, payer_key) =
            self.process_transaction(&sanitized_tx, compute_budget_limits);
        if let Some(ctx) = context {
            let signature = sanitized_tx.signature().to_owned();
            let ExecutionRecord {
                accounts,
                return_data,
                touched_account_count: _,
                accounts_resize_delta: _,
            } = ctx.into();
            let msg = sanitized_tx.message();
            let post_accounts = accounts
                .into_iter()
                .enumerate()
                .filter_map(|(idx, pair)| msg.is_writable(idx).then_some(pair))
                .collect();
            let tx_result = if result.is_ok() {
                result
            } else if let Some(payer) = payer_key {
                self.accounts.withdraw(&payer, fee).and(result)
            } else {
                result
            };

            ExecutionResult {
                tx_result,
                signature,
                post_accounts,
                compute_units_consumed,
                return_data,
                included: true,
            }
        } else {
            ExecutionResult {
                tx_result: result,
                compute_units_consumed,
                ..Default::default()
            }
        }
    }

    fn execute_transaction_readonly(&self, tx: VersionedTransaction) -> ExecutionResult {
        let sanitized_tx = match self.sanitize_transaction(tx) {
            Ok(s_tx) => s_tx,
            Err(err) => {
                return ExecutionResult {
                    tx_result: Err(err),
                    ..Default::default()
                }
            }
        };
        self.execute_sanitized_transaction_readonly(sanitized_tx)
    }

    fn execute_transaction_no_verify_readonly(&self, tx: VersionedTransaction) -> ExecutionResult {
        let sanitized_tx = match self.sanitize_transaction_no_verify(tx) {
            Ok(s_tx) => s_tx,
            Err(err) => {
                return ExecutionResult {
                    tx_result: Err(err),
                    ..Default::default()
                };
            }
        };
        self.execute_sanitized_transaction_readonly(sanitized_tx)
    }

    fn execute_sanitized_transaction_readonly(
        &self,
        sanitized_tx: SanitizedTransaction,
    ) -> ExecutionResult {
        if self.blockhash_check {
            if let Err(e) = self.check_transaction_age(&sanitized_tx) {
                return ExecutionResult {
                    tx_result: Err(e),
                    ..Default::default()
                };
            }
        }
        let instructions = sanitized_tx.message().program_instructions_iter();
        let compute_budget_limits = match process_compute_budget_instructions(instructions) {
            Ok(x) => x,
            Err(e) => {
                return ExecutionResult {
                    tx_result: Err(e),
                    ..Default::default()
                };
            }
        };
        if self.history.check_transaction(sanitized_tx.signature()) {
            return ExecutionResult {
                tx_result: Err(TransactionError::AlreadyProcessed),
                ..Default::default()
            };
        }

        let (result, compute_units_consumed, context, _, _) =
            self.process_transaction(&sanitized_tx, compute_budget_limits);
        if let Some(ctx) = context {
            let signature = sanitized_tx.signature().to_owned();
            let ExecutionRecord {
                accounts,
                return_data,
                touched_account_count: _,
                accounts_resize_delta: _,
            } = ctx.into();
            let msg = sanitized_tx.message();
            let post_accounts = accounts
                .into_iter()
                .enumerate()
                .filter_map(|(idx, pair)| msg.is_writable(idx).then_some(pair))
                .collect();

            ExecutionResult {
                tx_result: result,
                signature,
                post_accounts,
                compute_units_consumed,
                return_data,
                included: true,
            }
        } else {
            ExecutionResult {
                tx_result: result,
                compute_units_consumed,
                ..Default::default()
            }
        }
    }

    pub(crate) fn send_message<T: Signers>(
        &mut self,
        message: Message,
        signers: &T,
    ) -> TransactionResult {
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), signers).unwrap();
        self.send_transaction(tx)
    }

    pub fn send_transaction(&mut self, tx: impl Into<VersionedTransaction>) -> TransactionResult {
        let vtx: VersionedTransaction = tx.into();
        let ExecutionResult {
            post_accounts,
            tx_result,
            signature,
            compute_units_consumed,
            return_data,
            included,
        } = if self.sigverify {
            self.execute_transaction(vtx)
        } else {
            self.execute_transaction_no_verify(vtx)
        };

        let meta = TransactionMetadata {
            logs: self.log_collector.take().into_messages(),
            compute_units_consumed,
            return_data,
            signature,
        };

        if let Err(tx_err) = tx_result {
            let err = TransactionResult::Err(FailedTransactionMetadata { err: tx_err, meta });
            if included {
                self.history.add_new_transaction(signature, err.clone());
            }
            err
        } else {
            self.history
                .add_new_transaction(signature, Ok(meta.clone()));
            self.accounts
                .sync_accounts(post_accounts)
                .expect("It shouldn't be possible to write invalid sysvars in send_transaction.");

            TransactionResult::Ok(meta)
        }
    }

    pub fn simulate_transaction(&self, tx: impl Into<VersionedTransaction>) -> TransactionResult {
        let ExecutionResult {
            post_accounts: _,
            tx_result,
            signature,
            compute_units_consumed,
            return_data,
            ..
        } = if self.sigverify {
            self.execute_transaction_readonly(tx.into())
        } else {
            self.execute_transaction_no_verify_readonly(tx.into())
        };

        let logs = self.log_collector.take().into_messages();
        let meta = TransactionMetadata {
            signature,
            logs,
            compute_units_consumed,
            return_data,
        };

        if let Err(tx_err) = tx_result {
            TransactionResult::Err(FailedTransactionMetadata { err: tx_err, meta })
        } else {
            TransactionResult::Ok(meta)
        }
    }

    pub fn expire_blockhash(&mut self) {
        self.latest_blockhash = create_blockhash(&self.latest_blockhash.to_bytes());
        #[allow(deprecated)]
        self.set_sysvar(&RecentBlockhashes::from_iter([IterItem(
            0,
            &self.latest_blockhash,
            self.fee_structure.lamports_per_signature,
        )]));
    }

    pub fn warp_to_slot(&mut self, slot: u64) {
        let mut clock = self.get_sysvar::<Clock>();
        clock.slot = slot;
        self.set_sysvar(&clock);
    }

    pub fn get_compute_budget(&self) -> Option<ComputeBudget> {
        self.compute_budget
    }

    pub fn set_compute_budget(&mut self, budget: ComputeBudget) {
        self.compute_budget = Some(budget);
    }

    #[cfg(feature = "internal-test")]
    pub fn get_feature_set(&self) -> Arc<FeatureSet> {
        self.feature_set.clone()
    }

    fn check_transaction_age(
        &self,
        tx: &SanitizedTransaction,
    ) -> solana_sdk::transaction::Result<()> {
        let recent_blockhash = tx.message().recent_blockhash();
        if recent_blockhash == &self.latest_blockhash
            || self.check_transaction_for_nonce(
                tx,
                &DurableNonce::from_blockhash(&self.latest_blockhash),
            )
        {
            Ok(())
        } else {
            log::error!(
                "Blockhash {} not found. Expected blockhash {}",
                recent_blockhash,
                self.latest_blockhash
            );
            Err(TransactionError::BlockhashNotFound)
        }
    }

    fn check_message_for_nonce(&self, message: &SanitizedMessage) -> bool {
        message
            .get_durable_nonce()
            .and_then(|nonce_address| self.accounts.get_account(nonce_address))
            .and_then(|nonce_account| {
                nonce_account::verify_nonce_account(&nonce_account, message.recent_blockhash())
            })
            .map_or(false, |nonce_data| {
                message
                    .get_ix_signers(NONCED_TX_MARKER_IX_INDEX as usize)
                    .any(|signer| signer == &nonce_data.authority)
            })
    }

    fn check_transaction_for_nonce(
        &self,
        tx: &SanitizedTransaction,
        next_durable_nonce: &DurableNonce,
    ) -> bool {
        let nonce_is_advanceable = tx.message().recent_blockhash() != next_durable_nonce.as_hash();
        nonce_is_advanceable && self.check_message_for_nonce(tx.message())
    }
}

/// Lighter version of the one in the solana-svm crate.
///
/// Check whether the payer_account is capable of paying the fee. The
/// side effect is to subtract the fee amount from the payer_account
/// balance of lamports. If the payer_acount is not able to pay the
/// fee a specific error is returned.
fn validate_fee_payer(
    payer_address: &Pubkey,
    payer_account: &mut AccountSharedData,
    payer_index: IndexOfAccount,
    rent: &Rent,
    fee: u64,
) -> solana_sdk::transaction::Result<()> {
    if payer_account.lamports() == 0 {
        error!("Payer account {payer_address} not found.");
        return Err(TransactionError::AccountNotFound);
    }
    let system_account_kind = get_system_account_kind(payer_account).ok_or_else(|| {
        error!("Payer account {payer_address} is not a system account");
        TransactionError::InvalidAccountForFee
    })?;
    let min_balance = match system_account_kind {
        SystemAccountKind::System => 0,
        SystemAccountKind::Nonce => {
            // Should we ever allow a fees charge to zero a nonce account's
            // balance. The state MUST be set to uninitialized in that case
            rent.minimum_balance(solana_sdk::nonce::State::size())
        }
    };

    let payer_lamports = payer_account.lamports();

    payer_lamports
        .checked_sub(min_balance)
        .and_then(|v| v.checked_sub(fee))
        .ok_or_else(|| {
            error!(
                "Payer account {payer_address} has insufficient lamports for fee. Payer lamports: \
                {payer_lamports} min_balance: {min_balance} fee: {fee}"
            );
            TransactionError::InsufficientFundsForFee
        })?;

    let payer_pre_rent_state = RentState::from_account(payer_account, rent);
    // we already checked above if we have sufficient balance so this should never error.
    payer_account.checked_sub_lamports(fee).unwrap();

    let payer_post_rent_state = RentState::from_account(payer_account, rent);
    check_rent_state_with_account(
        &payer_pre_rent_state,
        &payer_post_rent_state,
        payer_address,
        payer_index,
    )
}

// modified version of the private fn in solana-svm
fn check_rent_state_with_account(
    pre_rent_state: &RentState,
    post_rent_state: &RentState,
    address: &Pubkey,
    account_index: IndexOfAccount,
) -> solana_sdk::transaction::Result<()> {
    if !solana_sdk::incinerator::check_id(address)
        && !post_rent_state.transition_allowed_from(pre_rent_state)
    {
        let account_index = account_index as u8;
        error!("Transaction would leave account {address} with insufficient funds for rent");
        Err(TransactionError::InsufficientFundsForRent { account_index })
    } else {
        Ok(())
    }
}
