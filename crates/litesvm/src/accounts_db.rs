#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(not(feature = "hashbrown"))]
use std::collections::HashMap;
use {
    crate::error::{InvalidSysvarDataError, LiteSVMError},
    jupnet_program_runtime::{
        loaded_programs::{
            LoadProgramMetrics, ProgramCacheEntry, ProgramCacheEntryOwner, ProgramCacheEntryType,
            ProgramCacheForTxBatch, ProgramRuntimeEnvironments,
        },
        sysvar_cache::SysvarCache,
    },
    jupnet_sdk::{
        account::{state_traits::StateMut, AccountSharedData, ReadableAccount, WritableAccount},
        bpf_loader, bpf_loader_deprecated,
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        clock::Clock,
        instruction::InstructionError,
        loader_v4, native_loader, nonce,
        pubkey::Pubkey,
        sysvar::{
            self, clock::ID as CLOCK_ID, epoch_rewards::ID as EPOCH_REWARDS_ID,
            epoch_schedule::ID as EPOCH_SCHEDULE_ID, last_restart_slot::ID as LAST_RESTART_SLOT_ID,
            rent::ID as RENT_ID, slot_hashes::ID as SLOT_HASHES_ID,
            stake_history::ID as STAKE_HISTORY_ID, Sysvar,
        },
    },
    jupnet_system_program::{get_system_account_kind, SystemAccountKind},
    jupnet_transaction_error::TransactionError,
    loader_v4::LoaderV4State,
    log::error,
    serde::de::DeserializeOwned,
    std::sync::Arc,
};

const FEES_ID: Pubkey = Pubkey::from_str_const("SysvarFees111111111111111111111111111111111");
const RECENT_BLOCKHASHES_ID: Pubkey =
    Pubkey::from_str_const("SysvarRecentB1ockHashes11111111111111111111");

fn handle_sysvar<T>(
    cache: &mut SysvarCache,
    err_variant: InvalidSysvarDataError,
    account: &AccountSharedData,
    accounts: &HashMap<Pubkey, AccountSharedData>,
    address: Pubkey,
) -> Result<(), InvalidSysvarDataError>
where
    T: Sysvar + DeserializeOwned,
{
    cache.reset();
    cache.fill_missing_entries(|pubkey, set_sysvar| {
        if *pubkey == address {
            set_sysvar(account.data())
        } else if let Some(acc) = accounts.get(pubkey) {
            set_sysvar(acc.data())
        }
    });
    let _parsed: T = bincode::deserialize(account.data()).map_err(|_| err_variant)?;
    Ok(())
}

#[derive(Clone, Default)]
pub struct AccountsDb {
    pub inner: HashMap<Pubkey, AccountSharedData>,
    pub programs_cache: ProgramCacheForTxBatch,
    pub sysvar_cache: SysvarCache,
    pub environments: ProgramRuntimeEnvironments,
}

impl AccountsDb {
    pub fn get_account_ref(&self, pubkey: &Pubkey) -> Option<&AccountSharedData> {
        self.inner.get(pubkey)
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.get_account_ref(pubkey).cloned()
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
        if account.lamports() == 0 {
            self.inner.remove(&pubkey);
        } else {
            self.add_account_no_checks(pubkey, account);
        }
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
                handle_sysvar::<sysvar::epoch_rewards::EpochRewards>(
                    cache,
                    EpochRewards,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            EPOCH_SCHEDULE_ID => {
                handle_sysvar::<sysvar::epoch_schedule::EpochSchedule>(
                    cache,
                    EpochSchedule,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            FEES_ID => {
                handle_sysvar::<sysvar::fees::Fees>(cache, Fees, account, &self.inner, pubkey)?;
            }
            LAST_RESTART_SLOT_ID => {
                handle_sysvar::<sysvar::last_restart_slot::LastRestartSlot>(
                    cache,
                    LastRestartSlot,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            RECENT_BLOCKHASHES_ID => {
                handle_sysvar::<sysvar::recent_blockhashes::RecentBlockhashes>(
                    cache,
                    RecentBlockhashes,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            RENT_ID => {
                handle_sysvar::<jupnet_sdk::rent::Rent>(cache, Rent, account, &self.inner, pubkey)?;
            }
            SLOT_HASHES_ID => {
                handle_sysvar::<jupnet_sdk::slot_hashes::SlotHashes>(
                    cache,
                    SlotHashes,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            STAKE_HISTORY_ID => {
                handle_sysvar::<jupnet_sdk::stake_history::StakeHistory>(
                    cache,
                    StakeHistory,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            _ => {}
        };
        Ok(())
    }

    /// Skip the executable() checks for builtin accounts
    pub(crate) fn add_builtin_account(&mut self, address: Pubkey, data: AccountSharedData) {
        self.inner.insert(address, data);
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
        for (address, acc) in accounts {
            self.add_account(address, acc)?;
        }
        Ok(())
    }

    fn load_program(
        &self,
        program_account: &AccountSharedData,
    ) -> Result<ProgramCacheEntry, InstructionError> {
        let metrics = &mut LoadProgramMetrics::default();

        let owner = program_account.owner();
        let program_runtime_v1 = self.environments.program_runtime_v1.clone();
        let slot = self.sysvar_cache.get_clock().unwrap().slot;

        if bpf_loader::check_id(owner) || bpf_loader_deprecated::check_id(owner) {
            ProgramCacheEntry::new(
                owner,
                program_runtime_v1,
                slot,
                slot,
                program_account.data(),
                program_account.data().len(),
                metrics,
            )
            .map_err(|e| {
                error!("Failed to load program: {e:?}");
                InstructionError::InvalidAccountData
            })
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
            let Some(programdata_account) = self.get_account_ref(&programdata_address) else {
                return Ok(ProgramCacheEntry::new_tombstone(
                    slot,
                    ProgramCacheEntryOwner::LoaderV3,
                    ProgramCacheEntryType::Closed,
                ));
            };
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
                    metrics).map_err(|e| {
                        error!("Error encountered when calling ProgramCacheEntry::new() for bpf_loader_upgradeable: {e:?}");
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

    pub(crate) fn withdraw(
        &mut self,
        address: &Pubkey,
        lamports: u64,
    ) -> Result<(), TransactionError> {
        match self.inner.get_mut(address) {
            Some(account) => {
                let min_balance = match get_system_account_kind(account) {
                    Some(SystemAccountKind::Nonce) => self
                        .sysvar_cache
                        .get_rent()
                        .unwrap()
                        .minimum_balance(nonce::state::State::size()),
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
                error!("Account {address} not found when trying to withdraw fee.");
                Err(TransactionError::AccountNotFound)
            }
        }
    }

    /// Returns a borrowed slice of ELF bytes for this account.
    /// Fails if the account is not a program account.
    pub fn try_program_elf_bytes<'a>(
        &'a self,
        program_key: &Pubkey,
    ) -> std::result::Result<&'a [u8], InstructionError> {
        let program_account = self
            .get_account_ref(program_key)
            .ok_or(InstructionError::MissingAccount)?;
        let owner = program_account.owner();

        if bpf_loader::check_id(owner) || bpf_loader_deprecated::check_id(owner) {
            Ok(program_account.data())
        } else if bpf_loader_upgradeable::check_id(owner) {
            let Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) = program_account.state()
            else {
                return Err(InstructionError::InvalidAccountData);
            };
            let programdata_account =
                self.get_account_ref(&programdata_address).ok_or_else(|| {
                    error!("Program data account {programdata_address} not found");
                    InstructionError::MissingAccount
                })?;
            let program_data = programdata_account.data();
            if let Some(programdata) =
                program_data.get(UpgradeableLoaderState::size_of_programdata_metadata()..)
            {
                Ok(programdata)
            } else {
                error!("Index out of bounds using bpf_loader_upgradeable.");
                Err(InstructionError::InvalidAccountData)
            }
        } else if loader_v4::check_id(owner) {
            if let Some(elf_bytes) = program_account
                .data()
                .get(LoaderV4State::program_data_offset()..)
            {
                Ok(elf_bytes)
            } else {
                error!("Index out of bounds using loader_v4.");
                Err(InstructionError::InvalidAccountData)
            }
        } else {
            error!("Owner does not match any expected loader.");
            Err(InstructionError::IncorrectProgramId)
        }
    }
}
