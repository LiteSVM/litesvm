use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_program_runtime::{
    compute_budget::ComputeBudget,
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
use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    create_blockhash,
    types::{ExecutionResult, TransactionMetadata, TransactionResult},
    utils::RentState,
    Error,
};

#[derive(Clone, Default)]
pub struct LightAddressLoader {}

impl AddressLoader for LightAddressLoader {
    fn load_addresses(
        self,
        _lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        Err(AddressLoaderError::Disabled)
    }
}

pub struct LightBank {
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

impl Default for LightBank {
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

impl LightBank {
    pub fn new() -> Self {
        LightBank::default()
            .with_builtins()
            .with_lamports(1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL))
            .with_sysvars()
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

        let program_runtime = create_program_runtime_environment_v1(
            &feature_set,
            &ComputeBudget::default(),
            false,
            true,
        )
        .unwrap();

        self.programs_cache.environments.program_runtime_v1 = Arc::new(program_runtime);
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

    pub fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
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

    pub fn get_balance(&self, pubkey: &Pubkey) -> u64 {
        self.accounts.get_account(pubkey).lamports()
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

    pub fn airdrop(&mut self, pubkey: &Pubkey, lamports: u64) -> Result<(), Error> {
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

        self.send_transaction(tx)?;
        Ok(())
    }

    pub fn store_program(&mut self, program_id: Pubkey, program_bytes: &[u8]) {
        let program_len = program_bytes.len();
        let lamports = self.get_minimum_balance_for_rent_exemption(program_len);
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
    pub fn load_program(&self, program_id: &Pubkey) -> Result<LoadedProgram, InstructionError> {
        let program_account = self.accounts.get_account(program_id);
        let metrics = &mut LoadProgramMetrics::default();

        if !program_account.executable() {
            return Err(InstructionError::AccountNotExecutable);
        };

        let owner = program_account.owner();
        let program_runtime_v1 = self.programs_cache.environments.program_runtime_v1.clone();

        if bpf_loader::check_id(owner) | bpf_loader_deprecated::check_id(owner) {
            LoadedProgram::new(
                program_account.owner(),
                self.programs_cache.environments.program_runtime_v1.clone(),
                self.slot,
                self.slot,
                None,
                program_account.data(),
                program_account.data().len(),
                &mut LoadProgramMetrics::default(),
            )
            .map_err(|_| InstructionError::InvalidAccountData)
        } else if bpf_loader_upgradeable::check_id(program_account.owner()) {
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
                        program_account.owner(),
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

        Ok(tx)
    }

    //TODO rework it with process_transaction and another on and optimize
    fn process_transaction(
        &mut self,
        tx: &SanitizedTransaction,
        compute_budget: ComputeBudget,
        context: &mut TransactionContext,
    ) -> Result<(Result<(), TransactionError>, LoadedProgramsForTxBatch, u64), Error> {
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

        let tx_result = MessageProcessor::process_message(
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

        self.check_accounts_rent(tx, context)?;

        Ok((
            tx_result,
            programs_modified_by_tx,
            accumulated_consume_units,
        ))
    }

    //TODO self.rent
    fn check_accounts_rent(
        &mut self,
        tx: &SanitizedTransaction,
        context: &TransactionContext,
    ) -> Result<(), Error> {
        for index in 0..tx.message().account_keys().len() {
            if tx.message().is_writable(index) {
                let account = context
                    .get_account_at_index(index as IndexOfAccount)?
                    .borrow();
                let pubkey = context.get_key_of_account_at_index(index as IndexOfAccount)?;
                let rent = self.sysvar_cache.get_rent().unwrap_or_default();

                if !account.data().is_empty() {
                    let post_rent_state = RentState::from_account(&account, &rent);
                    let pre_rent_state =
                        RentState::from_account(&self.accounts.get_account(pubkey), &rent);

                    if !post_rent_state.transition_allowed_from(&pre_rent_state) {
                        return Err(TransactionError::InsufficientFundsForRent {
                            account_index: index as u8,
                        }
                        .into());
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_transaction(&mut self, tx: VersionedTransaction) -> Result<ExecutionResult, Error> {
        let compute_budget = ComputeBudget::default();
        let sanitized_tx = self.sanitize_transaction(tx)?;
        let mut context = self.create_transaction_context(&sanitized_tx, compute_budget);

        let (result, programs_modified, compute_units_consumed) =
            self.process_transaction(&sanitized_tx, compute_budget, &mut context)?;

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

        Ok(ExecutionResult {
            tx_result: result,
            programs_modified,
            post_accounts: accounts,
            compute_units_consumed,
            return_data,
        })
    }

    pub fn send_message<T: Signers>(
        &mut self,
        message: Message,
        signers: &T,
    ) -> Result<TransactionResult, Error> {
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), signers)?;
        self.send_transaction(tx)
    }
    pub fn find_loaded(&self, key: &Pubkey) -> Arc<LoadedProgram> {
        self.programs_cache.find(key).unwrap_or_default()
    }

    pub fn send_transaction(
        &mut self,
        tx: VersionedTransaction,
    ) -> Result<TransactionResult, Error> {
        let ExecutionResult {
            post_accounts,
            tx_result,
            programs_modified,
            compute_units_consumed,
            return_data,
        } = self.execute_transaction(tx)?;

        if tx_result.is_ok() {
            //TODO check if programs are program_owners
            self.accounts.sync_accounts(post_accounts);
            for program_id in programs_modified {
                let loaded_program = self.load_program(&program_id)?;
                self.programs_cache
                    .replenish(program_id, Arc::new(loaded_program));
            }
        }

        let logs = self.log_collector.take().into_messages();

        // We don't care about parallelized transaction. (1 transaction by slot)
        self.next_slot();

        Ok(TransactionResult {
            result: tx_result,
            metadata: TransactionMetadata {
                logs,
                compute_units_consumed,
                return_data,
            },
        })
    }

    pub fn simulate_transaction(
        &mut self,
        tx: VersionedTransaction,
    ) -> Result<TransactionResult, Error> {
        let ExecutionResult {
            post_accounts: _,
            tx_result,
            programs_modified: _,
            compute_units_consumed,
            return_data,
        } = self.execute_transaction(tx)?;

        let logs = self.log_collector.take().into_messages();

        Ok(TransactionResult {
            result: tx_result,
            metadata: TransactionMetadata {
                logs,
                compute_units_consumed,
                return_data,
            },
        })
    }

    fn next_slot(&mut self) {
        self.latest_blockhash = create_blockhash(&self.latest_blockhash.to_bytes());
        self.slot += 1;
        self.block_height += 1;
        self.programs_cache.set_slot_for_tests(self.slot);
    }
}
