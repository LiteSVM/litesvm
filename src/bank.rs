#![allow(clippy::result_large_err)]
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_loader_v4_program::create_program_runtime_environment_v2;
use solana_program_runtime::{
    compute_budget::ComputeBudget,
    invoke_context::BuiltinFunctionWithContext,
    loaded_programs::{LoadProgramMetrics, LoadedProgram, LoadedProgramsForTxBatch},
    log_collector::LogCollector,
    message_processor::MessageProcessor,
    sysvar_cache::SysvarCache,
    timings::ExecuteTimings,
};
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Clock,
    feature_set::FeatureSet,
    hash::Hash,
    instruction::InstructionError,
    loader_v4::{self, LoaderV4State},
    message::{
        v0::{LoadedAddresses, MessageAddressTableLookup},
        AddressLoader, AddressLoaderError, Message, VersionedMessage,
    },
    native_loader,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    signers::Signers,
    slot_history::Slot,
    system_instruction, system_program,
    sysvar::{Sysvar, SysvarId},
    transaction::{MessageHash, SanitizedTransaction, TransactionError, VersionedTransaction},
    transaction_context::{ExecutionRecord, IndexOfAccount, TransactionContext},
};
use std::{cell::RefCell, default, rc::Rc, sync::Arc};

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    create_blockhash,
    spl::load_spl_programs,
    types::{ExecutionResult, FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    utils::RentState,
};

#[derive(Clone, Default)]
pub(crate) struct LightAddressLoader {}

impl AddressLoader for LightAddressLoader {
    fn load_addresses(
        self,
        _lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        Err(AddressLoaderError::Disabled)
    }
}

pub struct LiteSVM {
    accounts: AccountsDb,
    //TODO compute budget
    programs_cache: LoadedProgramsForTxBatch,
    airdrop_kp: Keypair,
    sysvar_cache: SysvarCache,
    feature_set: Arc<FeatureSet>,
    block_height: u64,
    slot: Slot,
    latest_blockhash: Hash,
    log_collector: Rc<RefCell<LogCollector>>,
}

impl Default for LiteSVM {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            programs_cache: Default::default(),
            airdrop_kp: Keypair::new(),
            sysvar_cache: Default::default(),
            feature_set: Default::default(),
            block_height: 0,
            slot: 0,
            latest_blockhash: create_blockhash(b"genesis"),
            log_collector: Default::default(),
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
    }

    pub fn with_sysvars(mut self) -> Self {
        self.set_sysvar(&Clock::default());
        self.set_sysvar(&Rent::default());
        self
    }

    pub fn with_builtins(mut self) -> Self {
        let mut feature_set = FeatureSet::all_enabled();

        BUILTINS.iter().for_each(|builtint| {
            let loaded_program =
                LoadedProgram::new_builtin(0, builtint.name.len(), builtint.entrypoint);
            self.programs_cache
                .replenish(builtint.program_id, Arc::new(loaded_program));
            self.accounts.add_account(
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

        self.programs_cache.environments.program_runtime_v1 = Arc::new(program_runtime_v1);
        self.programs_cache.environments.program_runtime_v2 = Arc::new(program_runtime_v2);
        self.feature_set = Arc::new(feature_set);
        self
    }

    pub fn with_lamports(mut self, lamports: u64) -> Self {
        self.accounts.add_account(
            self.airdrop_kp.pubkey(),
            AccountSharedData::new(lamports, 0, &system_program::id()),
        );
        self
    }

    pub fn with_spl_programs(mut self) -> Self {
        load_spl_programs(&mut self);
        self
    }

    pub fn minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
        1.max(
            self.sysvar_cache
                .get_rent()
                .unwrap_or_default()
                .minimum_balance(data_len),
        )
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Account {
        self.accounts.get_account(pubkey).into()
    }

    pub fn set_account(&mut self, pubkey: Pubkey, data: AccountSharedData) {
        self.accounts.add_account(pubkey, data)
    }

    pub fn get_balance(&self, pubkey: &Pubkey) -> u64 {
        self.accounts.get_account(pubkey).lamports()
    }

    pub fn latest_blockhash(&self) -> Hash {
        self.latest_blockhash
    }

    pub fn slot(&self) -> u64 {
        self.slot
    }

    pub fn set_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        let Ok(data) = bincode::serialize(sysvar) else {
            return;
        };

        let account = AccountSharedData::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();

        if T::id() == Clock::id() {
            if let Ok(clock) = bincode::deserialize(&data) {
                self.sysvar_cache.set_clock(clock);
                self.accounts.add_account(Clock::id(), account);
            }
        } else if T::id() == Rent::id() {
            if let Ok(rent) = bincode::deserialize(&data) {
                self.sysvar_cache.set_rent(rent);
                self.accounts.add_account(Rent::id(), account);
            }
        }
    }

    pub fn airdrop(&mut self, pubkey: &Pubkey, lamports: u64) -> TransactionResult {
        let payer = &self.airdrop_kp;
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(Message::new(
                &[system_instruction::transfer(
                    &payer.pubkey(),
                    pubkey,
                    lamports,
                )],
                Some(&payer.pubkey()),
            )),
            &[payer],
        )
        .unwrap();

        self.send_transaction(tx)
    }

    pub fn add_builtin(&mut self, program_id: Pubkey, entrypoint: BuiltinFunctionWithContext) {
        let builtin = LoadedProgram::new_builtin(self.slot, 1, entrypoint);

        self.programs_cache.replenish(program_id, Arc::new(builtin));
        self.accounts
            .add_account(program_id, AccountSharedData::new(0, 1, &bpf_loader::id()));
    }

    pub fn store_program(&mut self, program_id: Pubkey, program_bytes: &[u8]) {
        let program_len = program_bytes.len();
        let lamports = self.minimum_balance_for_rent_exemption(program_len);
        let mut account = AccountSharedData::new(lamports, program_len, &bpf_loader::id());
        account.set_executable(true);
        account.set_data_from_slice(program_bytes);

        let loaded_program = solana_bpf_loader_program::load_program_from_bytes(
            false,
            Some(self.log_collector.clone()),
            &mut LoadProgramMetrics::default(),
            account.data(),
            account.owner(),
            account.data().len(),
            self.slot,
            self.programs_cache.environments.program_runtime_v1.clone(),
            false,
        )
        .unwrap_or_default();
        self.accounts.add_account(program_id, account);
        self.programs_cache
            .replenish(program_id, Arc::new(loaded_program));
    }

    //TODO handle reload
    pub(crate) fn load_program(
        &self,
        program_id: &Pubkey,
    ) -> Result<LoadedProgram, InstructionError> {
        let program_account = self.accounts.get_account(program_id);
        let metrics = &mut LoadProgramMetrics::default();

        if !program_account.executable() {
            return Err(InstructionError::AccountNotExecutable);
        };

        let owner = program_account.owner();
        let program_runtime_v1 = self.programs_cache.environments.program_runtime_v1.clone();

        if bpf_loader::check_id(owner) | bpf_loader_deprecated::check_id(owner) {
            LoadedProgram::new(
                owner,
                self.programs_cache.environments.program_runtime_v1.clone(),
                self.slot,
                self.slot,
                None,
                program_account.data(),
                program_account.data().len(),
                &mut LoadProgramMetrics::default(),
            )
            .map_err(|_| InstructionError::InvalidAccountData)
        } else if bpf_loader_upgradeable::check_id(owner) {
            let Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) = program_account.state()
            else {
                return Err(InstructionError::InvalidAccountData);
            };
            let programdata_account = self.accounts.get_account(&programdata_address);

            programdata_account
                .data()
                .get(UpgradeableLoaderState::size_of_programdata_metadata()..)
                .ok_or(Box::new(InstructionError::InvalidAccountData).into())
                .and_then(|programdata| {
                    LoadedProgram::new(
                        owner,
                        program_runtime_v1,
                        self.slot,
                        self.slot,
                        None,
                        programdata,
                        program_account
                            .data()
                            .len()
                            .saturating_add(programdata_account.data().len()),
                        metrics,
                    )
                })
                .map_err(|_| InstructionError::InvalidAccountData)
        } else if loader_v4::check_id(owner) {
            program_account
                .data()
                .get(LoaderV4State::program_data_offset()..)
                .ok_or(Box::new(InstructionError::InvalidAccountData).into())
                .and_then(|elf_bytes| {
                    LoadedProgram::new(
                        &loader_v4::id(),
                        program_runtime_v1,
                        self.slot,
                        self.slot,
                        None,
                        elf_bytes,
                        program_account.data().len(),
                        metrics,
                    )
                })
                .map_err(|_| InstructionError::InvalidAccountData)
        } else {
            Err(InstructionError::IncorrectProgramId)
        }
    }

    //TODO
    fn create_transaction_context(
        &mut self,
        tx: &SanitizedTransaction,
        compute_budget: ComputeBudget,
    ) -> TransactionContext {
        let accounts: Vec<(Pubkey, AccountSharedData)> = tx
            .message()
            .account_keys()
            .iter()
            .map(|p| (*p, self.accounts.get_account(p)))
            .collect();

        TransactionContext::new(
            accounts,
            Some(Rent::default()), //TODO remove rent in future
            compute_budget.max_invoke_stack_height,
            compute_budget.max_instruction_trace_length,
        )
    }

    fn sanitize_transaction(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        let tx = SanitizedTransaction::try_create(
            tx,
            MessageHash::Compute,
            Some(false),
            LightAddressLoader::default(), //TODO
        )?;

        tx.verify()?;
        tx.verify_precompiles(&self.feature_set)?;

        Ok(tx)
    }

    //TODO rework it with process_transaction and another on and optimize
    fn process_transaction(
        &mut self,
        tx: &SanitizedTransaction,
        compute_budget: ComputeBudget,
        context: &mut TransactionContext,
    ) -> (Result<(), TransactionError>, LoadedProgramsForTxBatch, u64) {
        let blockhash = tx.message().recent_blockhash();
        let mut programs_modified_by_tx =
            LoadedProgramsForTxBatch::new(self.slot, self.programs_cache.environments.clone());
        let mut programs_updated_only_for_global_cache = LoadedProgramsForTxBatch::default();
        let mut accumulated_consume_units = 0;

        let program_indices = tx
            .message()
            .instructions()
            .iter()
            .map(|c| vec![c.program_id_index as u16])
            .collect::<Vec<Vec<u16>>>();

        let mut tx_result = MessageProcessor::process_message(
            tx.message(),
            &program_indices,
            context,
            *self.sysvar_cache.get_rent().unwrap_or_default(),
            Some(self.log_collector.clone()),
            &self.programs_cache,
            &mut programs_modified_by_tx,
            &mut programs_updated_only_for_global_cache,
            self.feature_set.clone(),
            compute_budget,
            &mut ExecuteTimings::default(),
            &self.sysvar_cache,
            *blockhash,
            0,
            u64::MAX,
            &mut accumulated_consume_units,
        )
        .map(|_| ());

        if let Err(err) = self.check_accounts_rent(tx, context) {
            tx_result = Err(err);
        };

        (
            tx_result,
            programs_modified_by_tx,
            accumulated_consume_units,
        )
    }

    fn check_accounts_rent(
        &mut self,
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
                let rent = self.sysvar_cache.get_rent().unwrap_or_default();

                if !account.data().is_empty() {
                    let post_rent_state = RentState::from_account(&account, &rent);
                    let pre_rent_state =
                        RentState::from_account(&self.accounts.get_account(pubkey), &rent);

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

    fn execute_transaction(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        let compute_budget = ComputeBudget::default();
        let sanitized_tx = match self.sanitize_transaction(tx) {
            Ok(s_tx) => s_tx,
            Err(err) => {
                return ExecutionResult {
                    tx_result: Err(err),
                    ..default::Default::default()
                }
            }
        };
        let mut context = self.create_transaction_context(&sanitized_tx, compute_budget);

        let (result, programs_modified, compute_units_consumed) =
            self.process_transaction(&sanitized_tx, compute_budget, &mut context);

        let ExecutionRecord {
            accounts,
            return_data,
            touched_account_count: _,
            accounts_resize_delta: _,
        } = context.into();

        let programs_modified = accounts
            .iter()
            .filter_map(|(k, _)| {
                if programs_modified.find(k).is_some() {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();

        ExecutionResult {
            tx_result: result,
            programs_modified,
            post_accounts: accounts,
            compute_units_consumed,
            return_data,
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
        let ExecutionResult {
            post_accounts,
            tx_result,
            programs_modified,
            compute_units_consumed,
            return_data,
        } = self.execute_transaction(tx.into());

        if tx_result.is_ok() {
            //TODO check if programs are program_owners
            self.accounts.sync_accounts(post_accounts);
            for program_id in programs_modified {
                let Ok(loaded_program) = self.load_program(&program_id) else {
                    return TransactionResult::Err(FailedTransactionMetadata {
                        err: TransactionError::ProgramAccountNotFound,
                        meta: TransactionMetadata {
                            logs: self.log_collector.take().into_messages(),
                            compute_units_consumed,
                            return_data,
                        },
                    });
                };
                self.programs_cache
                    .replenish(program_id, Arc::new(loaded_program));
            }
        }

        TransactionResult::Ok(TransactionMetadata {
            logs: self.log_collector.take().into_messages(),
            compute_units_consumed,
            return_data,
        })
    }

    pub fn simulate_transaction(&mut self, tx: VersionedTransaction) -> TransactionResult {
        let ExecutionResult {
            post_accounts: _,
            tx_result,
            programs_modified: _,
            compute_units_consumed,
            return_data,
        } = self.execute_transaction(tx);

        let logs = self.log_collector.take().into_messages();
        let meta = TransactionMetadata {
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

    pub fn set_slot(&mut self, slot: u64) {
        self.latest_blockhash = create_blockhash(&self.latest_blockhash.to_bytes());
        self.slot = slot;
        self.block_height = slot;
        self.programs_cache.set_slot_for_tests(slot);
    }
}
