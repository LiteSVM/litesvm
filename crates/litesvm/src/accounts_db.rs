use log::error;
use solana_program::{
    address_lookup_table::{self, error::AddressLookupError, state::AddressLookupTable},
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Clock,
    instruction::InstructionError,
    loader_v4::{self, LoaderV4State},
    message::{
        v0::{LoadedAddresses, MessageAddressTableLookup},
        AddressLoader, AddressLoaderError,
    },
    sysvar::{
        clock::ID as CLOCK_ID, epoch_rewards::ID as EPOCH_REWARDS_ID,
        epoch_schedule::ID as EPOCH_SCHEDULE_ID, last_restart_slot::ID as LAST_RESTART_SLOT_ID,
        rent::ID as RENT_ID, slot_hashes::ID as SLOT_HASHES_ID,
        stake_history::ID as STAKE_HISTORY_ID, Sysvar,
    },
};
use solana_program_runtime::{
    loaded_programs::{LoadProgramMetrics, ProgramCacheEntry, ProgramCacheForTxBatch},
    sysvar_cache::SysvarCache,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount, WritableAccount},
    account_utils::StateMut,
    native_loader, nonce,
    pubkey::Pubkey,
    transaction::TransactionError,
};
use solana_system_program::{get_system_account_kind, SystemAccountKind};
use std::{collections::HashMap, sync::Arc};

use crate::error::{InvalidSysvarDataError, LiteSVMError};

const FEES_ID: Pubkey = solana_program::pubkey!("SysvarFees111111111111111111111111111111111");
const RECENT_BLOCKHASHES_ID: Pubkey =
    solana_program::pubkey!("SysvarRecentB1ockHashes11111111111111111111");

fn handle_sysvar<T>(
    cache: &mut SysvarCache,
    err_variant: InvalidSysvarDataError,
    account: &AccountSharedData,
    mut accounts_clone: HashMap<Pubkey, AccountSharedData>,
    address: Pubkey,
) -> Result<(), InvalidSysvarDataError>
where
    T: Sysvar,
{
    accounts_clone.insert(address, account.clone());
    cache.reset();
    cache.fill_missing_entries(|pubkey, set_sysvar| {
        if let Some(acc) = accounts_clone.get(pubkey) {
            set_sysvar(acc.data())
        }
    });
    let _parsed: T = bincode::deserialize(account.data()).map_err(|_| err_variant)?;
    Ok(())
}

#[derive(Default)]
pub(crate) struct AccountsDb {
    inner: HashMap<Pubkey, AccountSharedData>,
    pub(crate) programs_cache: ProgramCacheForTxBatch,
    pub(crate) sysvar_cache: SysvarCache,
}

impl AccountsDb {
    pub(crate) fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.inner.get(pubkey).map(|acc| acc.to_owned())
    }

    /// We should only use this when we know we're not touching any executable or sysvar accounts,
    /// or have already handled such cases.
    pub(crate) fn add_account_no_checks(&mut self, pubkey: Pubkey, account: AccountSharedData) {
        self.inner.insert(pubkey, account);
    }

    pub(crate) fn add_account(
        &mut self,
        pubkey: Pubkey,
        account: AccountSharedData,
    ) -> Result<(), LiteSVMError> {
        if account.executable()
            && pubkey != Pubkey::default()
            && account.owner() != &native_loader::ID
        {
            let loaded_program = self.load_program(&account)?;
            self.programs_cache
                .replenish(pubkey, Arc::new(loaded_program));
        } else {
            self.maybe_handle_sysvar_account(pubkey, &account)?;
        }
        self.add_account_no_checks(pubkey, account);
        Ok(())
    }

    fn maybe_handle_sysvar_account(
        &mut self,
        pubkey: Pubkey,
        account: &AccountSharedData,
    ) -> Result<(), InvalidSysvarDataError> {
        use InvalidSysvarDataError::{
            EpochRewards, EpochSchedule, Fees, LastRestartSlot, RecentBlockhashes, Rent,
            SlotHashes, StakeHistory,
        };
        let cache = &mut self.sysvar_cache;
        #[allow(deprecated)]
        match pubkey {
            CLOCK_ID => {
                let parsed: Clock = bincode::deserialize(account.data())
                    .map_err(|_| InvalidSysvarDataError::Clock)?;
                self.programs_cache.set_slot_for_tests(parsed.slot);
                let mut accounts_clone = self.inner.clone();
                accounts_clone.insert(pubkey, account.clone());
                cache.reset();
                cache.fill_missing_entries(|pubkey, set_sysvar| {
                    if let Some(acc) = accounts_clone.get(pubkey) {
                        set_sysvar(acc.data())
                    }
                });
            }
            EPOCH_REWARDS_ID => {
                handle_sysvar::<solana_sdk::epoch_rewards::EpochRewards>(
                    cache,
                    EpochRewards,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            EPOCH_SCHEDULE_ID => {
                handle_sysvar::<solana_sdk::epoch_schedule::EpochSchedule>(
                    cache,
                    EpochSchedule,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            FEES_ID => {
                handle_sysvar::<solana_sdk::sysvar::fees::Fees>(
                    cache,
                    Fees,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            LAST_RESTART_SLOT_ID => {
                handle_sysvar::<solana_sdk::sysvar::last_restart_slot::LastRestartSlot>(
                    cache,
                    LastRestartSlot,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            RECENT_BLOCKHASHES_ID => {
                handle_sysvar::<solana_sdk::sysvar::recent_blockhashes::RecentBlockhashes>(
                    cache,
                    RecentBlockhashes,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            RENT_ID => {
                handle_sysvar::<solana_sdk::rent::Rent>(
                    cache,
                    Rent,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            SLOT_HASHES_ID => {
                handle_sysvar::<solana_sdk::slot_hashes::SlotHashes>(
                    cache,
                    SlotHashes,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            STAKE_HISTORY_ID => {
                handle_sysvar::<solana_sdk::stake_history::StakeHistory>(
                    cache,
                    StakeHistory,
                    account,
                    self.inner.clone(),
                    pubkey,
                )?;
            }
            _ => {}
        };
        Ok(())
    }

    /// Skip the executable() checks for builtin accounts
    pub(crate) fn add_builtin_account(&mut self, pubkey: Pubkey, data: AccountSharedData) {
        self.inner.insert(pubkey, data);
    }

    pub(crate) fn sync_accounts(
        &mut self,
        mut accounts: Vec<(Pubkey, AccountSharedData)>,
    ) -> Result<(), LiteSVMError> {
        // need to add programdata accounts first if there are any
        itertools::partition(&mut accounts, |x| {
            x.1.owner() == &bpf_loader_upgradeable::id()
                && x.1.data().first().is_some_and(|byte| *byte == 3)
        });
        for (pubkey, acc) in accounts {
            self.add_account(pubkey, acc)?;
        }
        Ok(())
    }

    fn load_program(
        &self,
        program_account: &AccountSharedData,
    ) -> Result<ProgramCacheEntry, InstructionError> {
        let metrics = &mut LoadProgramMetrics::default();

        let owner = program_account.owner();
        let program_runtime_v1 = self.programs_cache.environments.program_runtime_v1.clone();
        let slot = self.sysvar_cache.get_clock().unwrap().slot;

        if bpf_loader::check_id(owner) | bpf_loader_deprecated::check_id(owner) {
            ProgramCacheEntry::new(
                owner,
                self.programs_cache.environments.program_runtime_v1.clone(),
                slot,
                slot,
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
                error!(
                    "Program account data does not deserialize to UpgradeableLoaderState::Program"
                );
                return Err(InstructionError::InvalidAccountData);
            };
            let programdata_account = self.get_account(&programdata_address).ok_or_else(|| {
                error!("Program data account {programdata_address} not found");
                InstructionError::MissingAccount
            })?;
            let program_data = programdata_account.data();
            if let Some(programdata) =
                program_data.get(UpgradeableLoaderState::size_of_programdata_metadata()..)
            {
                ProgramCacheEntry::new(
                    owner,
                    program_runtime_v1,
                    slot,
                    slot,
                    programdata,
                    program_account
                        .data()
                        .len()
                        .saturating_add(program_data.len()),
                    metrics).map_err(|_| {
                        error!("Error encountered when calling ProgramCacheEntry::new() for bpf_loader_upgradeable.");
                        InstructionError::InvalidAccountData
                    })
            } else {
                error!("Index out of bounds using bpf_loader_upgradeable.");
                Err(InstructionError::InvalidAccountData)
            }
        } else if loader_v4::check_id(owner) {
            if let Some(elf_bytes) = program_account
                .data()
                .get(LoaderV4State::program_data_offset()..)
            {
                ProgramCacheEntry::new(
                    &loader_v4::id(),
                    program_runtime_v1,
                    slot,
                    slot,
                    elf_bytes,
                    program_account.data().len(),
                    metrics,
                )
                .map_err(|_| {
                    error!("Error encountered when calling LoadedProgram::new() for loader_v4.");
                    InstructionError::InvalidAccountData
                })
            } else {
                error!("Index out of bounds using loader_v4.");
                Err(InstructionError::InvalidAccountData)
            }
        } else {
            error!("Owner does not match any expected loader.");
            Err(InstructionError::IncorrectProgramId)
        }
    }

    fn load_lookup_table_addresses(
        &self,
        address_table_lookup: &MessageAddressTableLookup,
    ) -> std::result::Result<LoadedAddresses, AddressLookupError> {
        let table_account = self
            .get_account(&address_table_lookup.account_key)
            .ok_or(AddressLookupError::LookupTableAccountNotFound)?;

        if table_account.owner() == &address_lookup_table::program::id() {
            let slot_hashes = self.sysvar_cache.get_slot_hashes().unwrap();
            let current_slot = self.sysvar_cache.get_clock().unwrap().slot;
            let lookup_table = AddressLookupTable::deserialize(table_account.data())
                .map_err(|_ix_err| AddressLookupError::InvalidAccountData)?;

            Ok(LoadedAddresses {
                writable: lookup_table.lookup(
                    current_slot,
                    &address_table_lookup.writable_indexes,
                    &slot_hashes,
                )?,
                readonly: lookup_table.lookup(
                    current_slot,
                    &address_table_lookup.readonly_indexes,
                    &slot_hashes,
                )?,
            })
        } else {
            Err(AddressLookupError::InvalidAccountOwner)
        }
    }

    pub(crate) fn withdraw(
        &mut self,
        pubkey: &Pubkey,
        lamports: u64,
    ) -> solana_sdk::transaction::Result<()> {
        match self.inner.get_mut(pubkey) {
            Some(account) => {
                let min_balance = match get_system_account_kind(account) {
                    Some(SystemAccountKind::Nonce) => self
                        .sysvar_cache
                        .get_rent()
                        .unwrap()
                        .minimum_balance(nonce::State::size()),
                    _ => 0,
                };

                lamports
                    .checked_add(min_balance)
                    .filter(|required_balance| *required_balance <= account.lamports())
                    .ok_or(TransactionError::InsufficientFundsForFee)?;
                account
                    .checked_sub_lamports(lamports)
                    .map_err(|_| TransactionError::InsufficientFundsForFee)?;

                Ok(())
            }
            None => {
                error!("Account {pubkey} not found when trying to withdraw fee.");
                Err(TransactionError::AccountNotFound)
            }
        }
    }
}

fn into_address_loader_error(err: AddressLookupError) -> AddressLoaderError {
    match err {
        AddressLookupError::LookupTableAccountNotFound => {
            AddressLoaderError::LookupTableAccountNotFound
        }
        AddressLookupError::InvalidAccountOwner => AddressLoaderError::InvalidAccountOwner,
        AddressLookupError::InvalidAccountData => AddressLoaderError::InvalidAccountData,
        AddressLookupError::InvalidLookupIndex => AddressLoaderError::InvalidLookupIndex,
    }
}

impl AddressLoader for &AccountsDb {
    fn load_addresses(
        self,
        lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        lookups
            .iter()
            .map(|lookup| {
                self.load_lookup_table_addresses(lookup)
                    .map_err(into_address_loader_error)
            })
            .collect()
    }
}
