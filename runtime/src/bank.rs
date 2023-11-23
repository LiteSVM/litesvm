use solana_program::{
    message::{
        v0::{LoadedAddresses, MessageAddressTableLookup},
        AddressLoader, AddressLoaderError, Message, VersionedMessage,
    },
    native_token::LAMPORTS_PER_SOL,
    system_instruction,
};
use solana_program_runtime::{
    compute_budget::ComputeBudget,
    loaded_programs::{LoadedProgram, LoadedProgramsForTxBatch},
    log_collector::LogCollector,
    message_processor::MessageProcessor,
    sysvar_cache::SysvarCache,
    timings::ExecuteTimings,
};
use solana_sdk::{
    account::{Account, AccountSharedData},
    feature_set::FeatureSet,
    native_loader,
    pubkey::Pubkey,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    slot_history::Slot,
    system_program,
    sysvar::{Sysvar, SysvarId},
    transaction::{MessageHash, SanitizedTransaction, TransactionError, VersionedTransaction},
    transaction_context::{TransactionContext, TransactionReturnData},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    types::{TransactionMetadata, TransactionResult},
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
    loaded_programs: HashMap<Pubkey, Arc<LoadedProgram>>, //TODO: Maybe with LoadedPrograms
    loaded_programs_cache_for_tx: RefCell<LoadedProgramsForTxBatch>,
    airdrop_kp: Keypair,
    sysvar_cache: SysvarCache,
    feature_set: Arc<FeatureSet>,
    slot: Slot,
    log_collector: Rc<RefCell<LogCollector>>,
}

impl Default for LightBank {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            loaded_programs: Default::default(),
            loaded_programs_cache_for_tx: Default::default(),
            airdrop_kp: Keypair::new(),
            sysvar_cache: Default::default(),
            feature_set: Default::default(),
            slot: Default::default(),
            log_collector: Default::default(),
        }
    }
}

impl LightBank {
    pub fn new() -> Self {
        let mut light_bank = LightBank::default();

        //TODO sysvar
        //TODO feature
        let mut feature_set = FeatureSet::default();

        // light_bank.feature_set.activate(feature_id, slot)
        BUILTINS.iter().for_each(|builtint| {
            let loaded_program =
                LoadedProgram::new_builtin(0, builtint.name.len(), builtint.entrypoint);
            light_bank
                .loaded_programs
                .insert(builtint.program_id, Arc::new(loaded_program));
            light_bank.accounts.add_account(
                builtint.program_id,
                native_loader::create_loadable_account_for_test(builtint.name),
            );

            if let Some(feature_id) = builtint.feature_id {
                feature_set.activate(&feature_id, 0);
            }
        });

        light_bank.accounts.add_account(
            light_bank.airdrop_kp.pubkey(),
            AccountSharedData::new(
                1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL),
                0,
                &system_program::id(),
            ),
        );

        let rent_data =
            AccountSharedData::new_data(1, &Rent::default(), &solana_sdk::sysvar::id()).unwrap();
        light_bank.accounts.add_account(Rent::id(), rent_data);
        // sysvar_cache.set_clock(clock)
        light_bank.sysvar_cache.set_rent(Rent::default());

        light_bank
    }

    pub fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
        Rent::default().minimum_balance(data_len).max(1)
    }

    pub fn get_account(&self, pubkey: Pubkey) -> Account {
        self.accounts.get_account(&pubkey).into()
    }

    pub fn set_sysvar<T>(&self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        // self.sysvar_cache.
        //self.sysvar_cache.set_last_restart_slot(last_restart_slot)
    }

    pub fn airdrop(&self, pubkey: &Pubkey, lamports: u64) -> Result<(), Error> {
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

        self.execute_transaction(tx)?;
        Ok(())
    }

    fn replenish_program_cache(&self) {
        let mut loaded_programs_cache_for_tx = self.loaded_programs_cache_for_tx.borrow_mut();

        if self.slot >= loaded_programs_cache_for_tx.slot() {
            for (program_key, loaded_program) in &self.loaded_programs {
                if self.slot >= loaded_program.effective_slot {
                    loaded_programs_cache_for_tx.replenish(*program_key, loaded_program.clone());
                }
            }
        }
    }

    //TODO
    fn create_transaction_context(
        &self,
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
    ) -> Result<SanitizedTransaction, Error> {
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
        &self,
        tx: &SanitizedTransaction,
        compute_budget: ComputeBudget,
        context: &mut TransactionContext,
    ) -> Result<(Result<(), TransactionError>, u64), Error> {
        let blockhash = tx.message().recent_blockhash();
        let mut programs_modified_by_tx = LoadedProgramsForTxBatch::default();
        let mut programs_updated_only_for_global_cache = LoadedProgramsForTxBatch::default();
        let mut accumulated_consume_units = 0;

        self.replenish_program_cache();

        //TODO optimize
        let program_indices = [self
            .loaded_programs
            .keys()
            .filter_map(|prog_key| context.find_index_of_program_account(prog_key))
            .collect()];

        let tx_result = MessageProcessor::process_message(
            tx.message(),
            &program_indices,
            context,
            Rent::default(),
            Some(self.log_collector.clone()),
            &self.loaded_programs_cache_for_tx.borrow(),
            &mut programs_modified_by_tx,
            &mut programs_updated_only_for_global_cache,
            self.feature_set.clone(),
            compute_budget,
            &mut ExecuteTimings::default(),
            &self.sysvar_cache,
            *blockhash,
            0,
            0,
            &mut accumulated_consume_units,
        )
        .map(|_| ());

        Ok((tx_result, accumulated_consume_units))
    }

    pub fn execute_transaction(
        &self,
        tx: impl Into<VersionedTransaction>,
    ) -> Result<TransactionResult, Error> {
        let compute_budget = ComputeBudget::default();
        let sanitized_tx = self.sanitize_transaction(tx.into())?;
        let mut context = self.create_transaction_context(&sanitized_tx, compute_budget);

        let (result, compute_units_consumed) =
            self.process_transaction(&sanitized_tx, compute_budget, &mut context)?;

        self.accounts.sync_accounts(&context)?;

        let return_data = context.get_return_data();
        let return_data = TransactionReturnData {
            data: return_data.1.to_vec(),
            program_id: return_data.0.to_owned(),
        };
        let logs = self.log_collector.borrow().get_recorded_content().to_vec();

        Ok(TransactionResult {
            result,
            metadata: TransactionMetadata {
                logs,
                compute_units_consumed,
                return_data,
            },
        })
    }

    pub fn simulate_transaction(
        &self,
        tx: impl Into<VersionedTransaction>,
    ) -> Result<TransactionResult, Error> {
        let compute_budget = ComputeBudget::default(); //TODO
        let sanitized_tx = self.sanitize_transaction(tx.into())?;
        let mut context = self.create_transaction_context(&sanitized_tx, compute_budget);

        let (result, compute_units_consumed) =
            self.process_transaction(&sanitized_tx, ComputeBudget::default(), &mut context)?;

        let return_data = context.get_return_data();
        let return_data = TransactionReturnData {
            data: return_data.1.to_vec(),
            program_id: return_data.0.to_owned(),
        };
        let logs = self.log_collector.borrow().get_recorded_content().to_vec();

        Ok(TransactionResult {
            result,
            metadata: TransactionMetadata {
                logs,
                compute_units_consumed,
                return_data,
            },
        })
    }
}
