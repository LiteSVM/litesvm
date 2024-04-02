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
        self, clock::ID as CLOCK_ID, epoch_rewards::ID as EPOCH_REWARDS_ID,
        epoch_schedule::ID as EPOCH_SCHEDULE_ID, last_restart_slot::ID as LAST_RESTART_SLOT_ID,
        rent::ID as RENT_ID, slot_hashes::ID as SLOT_HASHES_ID,
        stake_history::ID as STAKE_HISTORY_ID, Sysvar,
    },
};
use solana_program_runtime::{
    loaded_programs::{LoadProgramMetrics, LoadedProgram, LoadedProgramsForTxBatch},
    sysvar_cache::SysvarCache,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount, WritableAccount},
    account_utils::StateMut,
    nonce,
    pubkey::Pubkey,
    transaction::TransactionError,
};
use solana_system_program::{get_system_account_kind, SystemAccountKind};
use std::{collections::HashMap, sync::Arc};

use crate::types::InvalidSysvarDataError;

const FEES_ID: Pubkey = solana_program::pubkey!("SysvarFees111111111111111111111111111111111");
const RECENT_BLOCKHASHES_ID: Pubkey =
    solana_program::pubkey!("SysvarRecentB1ockHashes11111111111111111111");

fn handle_sysvar<F, T>(
    cache: &mut SysvarCache,
    method: F,
    err_variant: InvalidSysvarDataError,
    bytes: &[u8],
) -> Result<(), InvalidSysvarDataError>
where
    T: Sysvar,
    F: Fn(&mut SysvarCache, T),
{
    method(
        cache,
        bincode::deserialize::<T>(bytes).map_err(|_| err_variant)?,
    );
    Ok(())
}

#[derive(Default)]
pub(crate) struct AccountsDb {
    inner: HashMap<Pubkey, AccountSharedData>,
    pub(crate) programs_cache: LoadedProgramsForTxBatch,
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
    ) -> Result<(), InvalidSysvarDataError> {
        if account.executable() && pubkey != Pubkey::default() {
            let loaded_program = self.load_program(&account).unwrap();
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
                cache.set_clock(parsed);
            }
            EPOCH_REWARDS_ID => {
                handle_sysvar(
                    cache,
                    SysvarCache::set_epoch_rewards,
                    EpochRewards,
                    account.data(),
                )?;
            }
            EPOCH_SCHEDULE_ID => {
                handle_sysvar(
                    cache,
                    SysvarCache::set_epoch_schedule,
                    EpochSchedule,
                    account.data(),
                )?;
            }
            FEES_ID => {
                handle_sysvar(cache, SysvarCache::set_fees, Fees, account.data())?;
            }
            LAST_RESTART_SLOT_ID => {
                handle_sysvar(
                    cache,
                    SysvarCache::set_last_restart_slot,
                    LastRestartSlot,
                    account.data(),
                )?;
            }
            RECENT_BLOCKHASHES_ID => {
                handle_sysvar(
                    cache,
                    SysvarCache::set_recent_blockhashes,
                    RecentBlockhashes,
                    account.data(),
                )?;
            }
            RENT_ID => {
                handle_sysvar(cache, SysvarCache::set_rent, Rent, account.data())?;
            }
            SLOT_HASHES_ID => {
                handle_sysvar(
                    cache,
                    SysvarCache::set_slot_hashes,
                    SlotHashes,
                    account.data(),
                )?;
            }
            STAKE_HISTORY_ID => {
                handle_sysvar(
                    cache,
                    SysvarCache::set_stake_history,
                    StakeHistory,
                    account.data(),
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
    ) -> Result<(), InvalidSysvarDataError> {
        // need to add programdata accounts first if there are any
        itertools::partition(&mut accounts, |x| {
            x.1.owner() == &bpf_loader_upgradeable::id()
                && x.1.data().first().map_or(false, |byte| *byte == 3)
        });
        for (pubkey, acc) in accounts {
            self.add_account(pubkey, acc)?;
        }
        Ok(())
    }

    fn load_program(
        &self,
        program_account: &AccountSharedData,
        // programdata_account: Option<&AccountSharedData>
    ) -> Result<LoadedProgram, InstructionError> {
        let metrics = &mut LoadProgramMetrics::default();

        let owner = program_account.owner();
        let program_runtime_v1 = self.programs_cache.environments.program_runtime_v1.clone();
        let clock_acc = self.get_account(&sysvar::clock::ID);
        let clock: Clock = clock_acc
            .map(|x| bincode::deserialize::<Clock>(x.data()).unwrap())
            .unwrap_or_default();
        let slot = clock.slot;

        if bpf_loader::check_id(owner) | bpf_loader_deprecated::check_id(owner) {
            LoadedProgram::new(
                owner,
                self.programs_cache.environments.program_runtime_v1.clone(),
                slot,
                slot,
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
            let programdata_account = self.get_account(&programdata_address).unwrap();
            let program_data = programdata_account.data();
            program_data
                .get(UpgradeableLoaderState::size_of_programdata_metadata()..)
                .ok_or(Box::new(InstructionError::InvalidAccountData).into())
                .and_then(|programdata| {
                    LoadedProgram::new(
                        owner,
                        program_runtime_v1,
                        slot,
                        slot,
                        None,
                        programdata,
                        program_account
                            .data()
                            .len()
                            .saturating_add(program_data.len()),
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
                        slot,
                        slot,
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
            None => Err(TransactionError::AccountNotFound),
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
