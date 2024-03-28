use itertools::Itertools;
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_loader_v4_program::create_program_runtime_environment_v2;
#[allow(deprecated)]
use solana_program::sysvar::{fees::Fees, recent_blockhashes::RecentBlockhashes};
use solana_program::{
    message::SanitizedMessage,
    sysvar::{self, instructions::construct_instructions_data},
};
use solana_program_runtime::{
    compute_budget::ComputeBudget,
    invoke_context::BuiltinFunctionWithContext,
    loaded_programs::{LoadProgramMetrics, LoadedProgram, LoadedProgramsForTxBatch},
    log_collector::LogCollector,
    message_processor::MessageProcessor,
    timings::ExecuteTimings,
};
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    bpf_loader,
    clock::Clock,
    epoch_rewards::EpochRewards,
    epoch_schedule::EpochSchedule,
    feature_set::FeatureSet,
    hash::Hash,
    message::{Message, VersionedMessage},
    native_loader,
    native_token::LAMPORTS_PER_SOL,
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
use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    create_blockhash,
    history::TransactionHistory,
    spl::load_spl_programs,
    types::{
        ExecutionResult, FailedTransactionMetadata, InvalidSysvarDataError, TransactionMetadata,
        TransactionResult,
    },
    utils::RentState,
};

fn construct_instructions_account(message: &SanitizedMessage) -> AccountSharedData {
    AccountSharedData::from(Account {
        data: construct_instructions_data(&message.decompile_instructions()),
        owner: sysvar::id(),
        ..Account::default()
    })
}

pub struct LiteSVM {
    accounts: AccountsDb,
    airdrop_kp: Keypair,
    feature_set: Arc<FeatureSet>,
    latest_blockhash: Hash,
    log_collector: Rc<RefCell<LogCollector>>,
    history: TransactionHistory,
    compute_budget: Option<ComputeBudget>,
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

    pub fn with_compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        self.compute_budget = Some(compute_budget);
        self
    }

    pub fn with_sysvars(mut self) -> Self {
        self.set_sysvar(&Clock::default());
        self.set_sysvar(&EpochRewards::default());
        self.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        self.set_sysvar(&Fees::default());
        self.set_sysvar(&LastRestartSlot::default());
        #[allow(deprecated)]
        self.set_sysvar(&RecentBlockhashes::default());
        self.set_sysvar(&Rent::default());
        self.set_sysvar(&SlotHashes::new(&[(
            self.accounts.sysvar_cache.get_clock().unwrap().slot,
            self.latest_blockhash(),
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

    pub fn set_account(
        &mut self,
        pubkey: Pubkey,
        data: Account,
    ) -> Result<(), InvalidSysvarDataError> {
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

    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionMetadata> {
        self.history.get_transaction(signature)
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

    //TODO
    fn create_transaction_context(
        &mut self,
        compute_budget: ComputeBudget,
        accounts: Vec<(Pubkey, AccountSharedData)>,
    ) -> TransactionContext {
        TransactionContext::new(
            accounts,
            Rent::default(), //TODO remove rent in future
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
            &self.accounts,
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
    ) -> (Result<(), TransactionError>, u64, TransactionContext) {
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
        let mut accounts = account_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let mut account_found = true;
                #[allow(clippy::collapsible_else_if)]
                let account = if solana_sdk::sysvar::instructions::check_id(key) {
                    construct_instructions_account(message)
                } else {
                    let instruction_account = u8::try_from(i)
                        .map(|i| instruction_accounts.contains(&&i))
                        .unwrap_or(false);

                    if !instruction_account
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
                    }
                };

                (*key, account)
            })
            .collect::<Vec<_>>();
        let builtins_start_index = accounts.len();
        let program_indices = tx
            .message()
            .instructions()
            .iter()
            .map(|c| {
                let mut account_indices: Vec<u16> = Vec::with_capacity(2);
                let program_index = c.program_id_index as usize;
                // This command may never return error, because the transaction is sanitized
                let (program_id, program_account) = accounts.get(program_index).unwrap();
                if native_loader::check_id(program_id) {
                    return account_indices;
                }
                assert!(program_account.executable());
                account_indices.insert(0, program_index as IndexOfAccount);

                let owner_id = program_account.owner();
                if native_loader::check_id(owner_id) {
                    return account_indices;
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
                        assert!(native_loader::check_id(owner_account.owner()));
                        assert!(owner_account.executable);
                        accounts.push((*owner_id, owner_account.into()));
                        owner_index
                    },
                );
                account_indices
            })
            .collect::<Vec<Vec<u16>>>();
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

        (tx_result, accumulated_consume_units, context)
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
        let compute_budget = self.compute_budget.unwrap_or_else(|| {
            let instructions = sanitized_tx.message().program_instructions_iter();
            ComputeBudget::try_from_instructions(instructions).unwrap_or_default()
        });

        if self.history.check_transaction(sanitized_tx.signature()) {
            return ExecutionResult {
                tx_result: Err(TransactionError::AlreadyProcessed),
                ..Default::default()
            };
        }

        let (result, compute_units_consumed, context) =
            self.process_transaction(&sanitized_tx, compute_budget);
        let signature = sanitized_tx.signature().to_owned();
        let ExecutionRecord {
            accounts,
            return_data,
            touched_account_count: _,
            accounts_resize_delta: _,
        } = context.into();
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
        } = self.execute_transaction(vtx);

        let meta = TransactionMetadata {
            logs: self.log_collector.take().into_messages(),
            compute_units_consumed,
            return_data,
            signature,
        };

        if let Err(tx_err) = tx_result {
            TransactionResult::Err(FailedTransactionMetadata { err: tx_err, meta })
        } else {
            self.history
                .add_new_transaction(meta.signature, meta.clone());
            self.accounts
                .sync_accounts(post_accounts)
                .expect("It shouldn't be possible to write invalid sysvars in send_transaction.");

            TransactionResult::Ok(meta)
        }
    }

    pub fn simulate_transaction(&mut self, tx: VersionedTransaction) -> TransactionResult {
        let ExecutionResult {
            post_accounts: _,
            tx_result,
            signature,
            compute_units_consumed,
            return_data,
        } = self.execute_transaction(tx);

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
}
