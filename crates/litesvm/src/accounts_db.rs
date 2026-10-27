#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(not(feature = "hashbrown"))]
use std::collections::HashMap;
use {
    crate::error::{InvalidSysvarDataError, LiteSVMError},
    log::error,
    solana_account::{AccountSharedData, ReadableAccount, WritableAccount},
    solana_address::Address,
    solana_address_lookup_table_interface::{error::AddressLookupError, state::AddressLookupTable},
    solana_clock::Clock,
    solana_instruction_error::InstructionError,
    solana_loader_v3_interface::state::UpgradeableLoaderState,
    solana_loader_v4_interface::state::LoaderV4State,
    solana_message::{
        v0::{LoadedAddresses, MessageAddressTableLookup},
        AddressLoader,
    },
    solana_nonce as nonce,
    solana_program_runtime::{
        invoke_context::InvokeContext,
        loaded_programs::{
            ProgramCacheForTxBatch, ProgramRuntimeEnvironment, ProgramRuntimeEnvironments,
        },
        program_cache_entry::{
            ProgramCacheEntry, ProgramCacheEntryOwner, ProgramCacheEntryType,
            DELAY_VISIBILITY_SLOT_OFFSET,
        },
        program_metrics::LoadProgramMetrics,
        solana_sbpf::{elf::Executable, program::BuiltinProgram},
        sysvar_cache::SysvarCache,
    },
    solana_sdk_ids::{
        bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable, loader_v4, native_loader,
        sysvar::{
            clock::ID as CLOCK_ID, epoch_rewards::ID as EPOCH_REWARDS_ID,
            epoch_schedule::ID as EPOCH_SCHEDULE_ID, last_restart_slot::ID as LAST_RESTART_SLOT_ID,
            rent::ID as RENT_ID, slot_hashes::ID as SLOT_HASHES_ID,
            stake_history::ID as STAKE_HISTORY_ID,
        },
    },
    solana_system_program::{get_system_account_kind, SystemAccountKind},
    solana_sysvar::Sysvar,
    solana_transaction_error::{AddressLoaderError, TransactionError},
    std::sync::Arc,
    wincode::DeserializeOwned,
};

const FEES_ID: Address = Address::from_str_const("SysvarFees111111111111111111111111111111111");
const RECENT_BLOCKHASHES_ID: Address =
    Address::from_str_const("SysvarRecentB1ockHashes11111111111111111111");

fn handle_sysvar<T>(
    cache: &mut SysvarCache,
    err_variant: InvalidSysvarDataError,
    account: &AccountSharedData,
    accounts: &HashMap<Address, AccountSharedData>,
    address: Address,
) -> Result<T, InvalidSysvarDataError>
where
    T: Sysvar + DeserializeOwned<Dst = T>,
{
    let parsed = T::deserialize_from(account.data()).map_err(|_| err_variant)?;
    cache.reset();
    cache.fill_missing_entries(|pubkey, set_sysvar| {
        if *pubkey == address {
            set_sysvar(account.data())
        } else if let Some(acc) = accounts.get(pubkey) {
            set_sysvar(acc.data())
        }
    });
    Ok(parsed)
}

pub struct AccountsDb {
    pub inner: HashMap<Address, AccountSharedData>,
    pub programs_cache: ProgramCacheForTxBatch,
    pub sysvar_cache: SysvarCache,
    pub environments: ProgramRuntimeEnvironments,
}

impl Clone for AccountsDb {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            programs_cache: self.programs_cache.clone(),
            sysvar_cache: self.sysvar_cache.clone(),
            environments: ProgramRuntimeEnvironments::new(
                self.environments.get_env_for_execution().clone(),
                self.environments.get_env_for_deployment().clone(),
            ),
        }
    }
}

impl Default for AccountsDb {
    fn default() -> Self {
        let env = ProgramRuntimeEnvironment::from(
            BuiltinProgram::<InvokeContext<'static, 'static>>::new_mock(),
        );

        Self {
            inner: HashMap::default(),
            programs_cache: ProgramCacheForTxBatch::new(0),
            sysvar_cache: SysvarCache::default(),
            environments: ProgramRuntimeEnvironments::new(env.clone(), env),
        }
    }
}

impl AccountsDb {
    pub fn get_account_ref(&self, pubkey: &Address) -> Option<&AccountSharedData> {
        self.inner.get(pubkey)
    }

    pub fn get_account(&self, pubkey: &Address) -> Option<AccountSharedData> {
        self.get_account_ref(pubkey).cloned()
    }

    /// We should only use this when we know we're not touching any executable or sysvar accounts,
    /// or have already handled such cases.
    pub(crate) fn add_account_no_checks(&mut self, pubkey: Address, account: AccountSharedData) {
        self.inner.insert(pubkey, account);
    }

    pub(crate) fn add_account(
        &mut self,
        pubkey: Address,
        account: AccountSharedData,
    ) -> Result<(), LiteSVMError> {
        if account.executable()
            && pubkey != Address::default()
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
        pubkey: Address,
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
                let clock = handle_sysvar::<Clock>(
                    cache,
                    InvalidSysvarDataError::Clock,
                    account,
                    &self.inner,
                    pubkey,
                )?;
                self.programs_cache.set_slot_for_tests(clock.slot);
            }
            EPOCH_REWARDS_ID => {
                handle_sysvar::<solana_epoch_rewards::EpochRewards>(
                    cache,
                    EpochRewards,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            EPOCH_SCHEDULE_ID => {
                handle_sysvar::<solana_epoch_schedule::EpochSchedule>(
                    cache,
                    EpochSchedule,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            FEES_ID => {
                handle_sysvar::<solana_sysvar::fees::Fees>(
                    cache,
                    Fees,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            LAST_RESTART_SLOT_ID => {
                handle_sysvar::<solana_sysvar::last_restart_slot::LastRestartSlot>(
                    cache,
                    LastRestartSlot,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            RECENT_BLOCKHASHES_ID => {
                handle_sysvar::<solana_sysvar::recent_blockhashes::RecentBlockhashes>(
                    cache,
                    RecentBlockhashes,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            RENT_ID => {
                handle_sysvar::<solana_rent::Rent>(cache, Rent, account, &self.inner, pubkey)?;
            }
            SLOT_HASHES_ID => {
                handle_sysvar::<solana_slot_hashes::SlotHashes>(
                    cache,
                    SlotHashes,
                    account,
                    &self.inner,
                    pubkey,
                )?;
            }
            STAKE_HISTORY_ID => {
                handle_sysvar::<solana_stake_history::StakeHistory>(
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

    /// Rebuilds the sysvar cache from account data already present in `self.inner`.
    #[cfg(feature = "persistence-internal")]
    pub(crate) fn rebuild_sysvar_cache(&mut self) {
        self.sysvar_cache.reset();
        let accounts = &self.inner;
        self.sysvar_cache
            .fill_missing_entries(|pubkey, set_sysvar| {
                if let Some(acc) = accounts.get(pubkey) {
                    set_sysvar(acc.data())
                }
            });
        if let Ok(clock) = self.sysvar_cache.get_clock() {
            self.programs_cache.set_slot_for_tests(clock.slot);
        }
    }

    /// Scans all accounts for executable BPF programs and loads them into the program cache.
    #[cfg(feature = "persistence-internal")]
    pub(crate) fn load_all_existing_programs(&mut self) -> Result<(), LiteSVMError> {
        for (key, account) in self
            .inner
            .iter()
            .filter(|(_, acc)| acc.executable() && acc.owner() != &native_loader::ID)
        {
            let loaded = self.load_program(account)?;
            self.programs_cache.replenish(*key, Arc::new(loaded));
        }
        Ok(())
    }

    pub(crate) fn sync_accounts(
        &mut self,
        mut accounts: Vec<(Address, AccountSharedData)>,
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
        let slot = self.sysvar_cache.get_clock().map(|c| c.slot).unwrap_or(0);
        let Some(elf_bytes) = self.program_elf_bytes(program_account)? else {
            return Ok(ProgramCacheEntry::new_closed_tombstone(
                slot,
                ProgramCacheEntryOwner::LoaderV3,
            ));
        };
        ProgramCacheEntry::load(
            program_account.owner(),
            self.environments.get_env_for_execution().clone(),
            visible_deployment_slot(slot),
            elf_bytes,
            &mut LoadProgramMetrics::default(),
        )
        .map_err(|e| {
            error!("Failed to load program: {e:?}");
            InstructionError::InvalidAccountData
        })
    }

    /// The ELF bytes of `program_account`, or `None` for a loader-v3 program
    /// whose programdata account doesn't exist.
    fn program_elf_bytes<'a>(
        &'a self,
        program_account: &'a AccountSharedData,
    ) -> Result<Option<&'a [u8]>, InstructionError> {
        let owner = program_account.owner();
        if bpf_loader::check_id(owner) || bpf_loader_deprecated::check_id(owner) {
            return Ok(Some(program_account.data()));
        }
        let (data, offset) = if bpf_loader_upgradeable::check_id(owner) {
            let Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) = UpgradeableLoaderState::deserialize_from(program_account.data())
            else {
                error!(
                    "Program account data does not deserialize to UpgradeableLoaderState::Program"
                );
                return Err(InstructionError::InvalidAccountData);
            };
            let Some(programdata_account) = self.get_account_ref(&programdata_address) else {
                return Ok(None);
            };
            (
                programdata_account.data(),
                UpgradeableLoaderState::size_of_programdata_metadata(),
            )
        } else if loader_v4::check_id(owner) {
            (program_account.data(), LoaderV4State::program_data_offset())
        } else {
            error!("Owner does not match any expected loader.");
            return Err(InstructionError::IncorrectProgramId);
        };
        data.get(offset..).map(Some).ok_or_else(|| {
            error!("Program data of {owner} program is shorter than its header.");
            InstructionError::InvalidAccountData
        })
    }

    fn load_lookup_table_addresses(
        &self,
        address_table_lookup: &MessageAddressTableLookup,
    ) -> std::result::Result<LoadedAddresses, AddressLookupError> {
        let table_account = self
            .get_account_ref(&address_table_lookup.account_key)
            .ok_or(AddressLookupError::LookupTableAccountNotFound)?;

        if table_account.owner() == &solana_sdk_ids::address_lookup_table::id() {
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
        address: &Address,
        lamports: u64,
    ) -> solana_transaction_error::TransactionResult<()> {
        let Some(account) = self.inner.get_mut(address) else {
            error!("Account {address} not found when trying to withdraw fee.");
            return Err(TransactionError::AccountNotFound);
        };
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
            .map_err(|_| TransactionError::InsufficientFundsForFee)
    }

    /// Returns a borrowed slice of ELF bytes for this account.
    /// Fails if the account is not a program account.
    pub fn try_program_elf_bytes(&self, program_key: &Address) -> Result<&[u8], InstructionError> {
        let program_account = self
            .get_account_ref(program_key)
            .ok_or(InstructionError::MissingAccount)?;
        self.program_elf_bytes(program_account)?.ok_or_else(|| {
            error!("Program data account of {program_key} not found");
            InstructionError::MissingAccount
        })
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

/// Deployment slot that makes a freshly loaded program already effective at `current_slot`.
// ponytail: agave derives effective slot as deployment_slot + 1; wraps to u64::MAX at slot 0.
// The entry is hidden only if the clock is warped back to exactly this slot.
pub(crate) fn visible_deployment_slot(current_slot: u64) -> u64 {
    current_slot.wrapping_sub(DELAY_VISIBILITY_SLOT_OFFSET)
}

/// [`ProgramCacheEntry::load`] minus bytecode verification, for bundled programs
/// known to verify. Agave 4.3 dropped the equivalent `ProgramCacheEntry::reload`.
// ponytail: mirrors upstream `load` without `verify`; re-diff it on agave upgrades.
pub(crate) fn load_preverified(
    loader_key: &Address,
    environment: ProgramRuntimeEnvironment,
    deployment_slot: u64,
    elf_bytes: &[u8],
) -> Result<ProgramCacheEntry, Box<dyn std::error::Error>> {
    let executable = Executable::load(elf_bytes, Arc::clone(&*environment))?;
    #[cfg(all(not(target_os = "windows"), target_arch = "x86_64"))]
    executable.jit_compile()?;
    Ok(ProgramCacheEntry {
        program: ProgramCacheEntryType::Loaded(executable),
        account_owner: ProgramCacheEntryOwner::try_from(loader_key)
            .map_err(|()| format!("unsupported loader {loader_key}"))?,
        deployment_slot,
        stats: Default::default(),
        latest_access_slot: Default::default(),
    })
}
