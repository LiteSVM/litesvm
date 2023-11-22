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
    account::{Account, AccountSharedData, WritableAccount},
    bpf_loader, bpf_loader_upgradeable,
    feature_set::FeatureSet,
    native_loader,
    pubkey::Pubkey,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    slot_history::Slot,
    system_program,
    sysvar::{Sysvar, SysvarId},
    transaction::{MessageHash, SanitizedTransaction, VersionedTransaction},
    transaction_context::TransactionContext,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{Arc, RwLock},
};

use crate::{builtin::BUILTINS, Error};

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
    accounts: RefCell<HashMap<Pubkey, AccountSharedData>>,
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
        let sysvar_cache = SysvarCache::default();
        //TODO feature
        let mut feature_set = FeatureSet::default();

        // light_bank.feature_set.activate(feature_id, slot)
        BUILTINS.into_iter().for_each(|builtint| {
            let loaded_program =
                LoadedProgram::new_builtin(0, builtint.name.len(), builtint.entrypoint);
            light_bank
                .loaded_programs
                .insert(builtint.program_id, Arc::new(loaded_program));
            light_bank.accounts.borrow_mut().insert(
                builtint.program_id,
                native_loader::create_loadable_account_for_test(builtint.name),
            );

            if let Some(feature_id) = builtint.feature_id {
                feature_set.activate(&feature_id, 0);
            }
        });

        light_bank.accounts.borrow_mut().insert(
            light_bank.airdrop_kp.pubkey(),
            AccountSharedData::new(
                1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL),
                0,
                &system_program::id(),
            ),
        );
        // sysvar_cache.set_clock(clock)

        light_bank
    }

    pub fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
        Rent::default().minimum_balance(data_len).max(1)
    }

    // pub fn get_account(&self, pubkey: Pubkey) -> Option<Account> {
    //     self.accounts
    //         .borrow()
    //         .get(pubkey)
    //         .and_then(|acc| acc.into())
    // }

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

        let (sanitized_tx, mut context) = self.prepare_transaction(tx)?;

        self.execute_transaction(&sanitized_tx, &mut context)?;

        Ok(())
    }

    pub fn load_and_execute_transactions(&self, txs: &[SanitizedTransaction]) {}

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

    fn prepare_transaction(
        &self,
        tx: VersionedTransaction,
    ) -> Result<(SanitizedTransaction, TransactionContext), Error> {
        let compute_budget = ComputeBudget::default(); //TODO
        let sanitized_tx: SanitizedTransaction = SanitizedTransaction::try_create(
            tx,
            MessageHash::Compute,
            Some(false),
            LightAddressLoader::default(), //TODO
        )?;

        let accounts: Vec<(Pubkey, AccountSharedData)> = sanitized_tx
            .message()
            .account_keys()
            .iter()
            .map(|p| {
                (
                    *p,
                    self.accounts
                        .borrow()
                        .get(p)
                        .and_then(|acc| Some(acc.clone()))
                        .unwrap_or_default(),
                )
            })
            .collect();

        let mut transaction_context = TransactionContext::new(
            accounts,
            Some(Rent::default()), //TODO remove rent in future
            compute_budget.max_invoke_stack_height,
            compute_budget.max_instruction_trace_length,
        );

        Ok((sanitized_tx, transaction_context))
    }

    //TODO
    // fn create_transaction_context(&self, tx: &SanitizedTransaction) -> TransactionContext {}

    //TODO rework it with process_transaction and another one
    fn execute_transaction(
        &self,
        tx: &SanitizedTransaction,
        context: &mut TransactionContext,
    ) -> Result<(), Error> {
        let compute_budget = ComputeBudget::default();

        let blockhash = tx.message().recent_blockhash();

        self.replenish_program_cache();

        let mut programs_modified_by_tx = LoadedProgramsForTxBatch::default();
        let mut programs_updated_only_for_global_cache = LoadedProgramsForTxBatch::default();
        let mut accumulated_consume_units = 0;

        let program_indices = vec![vec![2]]; //TODO
        let process_result = MessageProcessor::process_message(
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
        )?;

        for index in 0..context.get_number_of_accounts() {
            let data = context.get_account_at_index(index)?;
            let pubkey = context.get_key_of_account_at_index(index)?;

            self.accounts
                .borrow_mut()
                .insert(*pubkey, data.borrow().to_owned());
        }
        // Ok(transaction_context.get_return_data().1)
        // Ok(context.get_return_data().1)
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use solana_program::{
        message::{Message, VersionedMessage},
        pubkey::Pubkey,
    };
    use solana_sdk::{signature::Keypair, signer::Signer, transaction::VersionedTransaction};

    use crate::bank::LightBank;

    //TODO make a correct test
    #[test]
    pub fn system_transfer() {
        let from_keypair = Keypair::new();
        let from = from_keypair.try_pubkey().unwrap();
        let to = Pubkey::new_unique();

        let instruction = solana_program::system_instruction::transfer(&from, &to, 64);
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(Message::new(&[instruction], Some(&from))),
            &[&from_keypair],
        )
        .unwrap();

        let light_bank = LightBank::new();

        light_bank.airdrop(&from, 100).unwrap();

        let (sanitized_tx, mut context) = light_bank.prepare_transaction(tx).unwrap();
        light_bank
            .execute_transaction(&sanitized_tx, &mut context)
            .unwrap();

        assert_eq!(1, 2);
    }
}
