use solana_program::{
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Clock,
    instruction::InstructionError,
    loader_v4::{self, LoaderV4State},
    sysvar,
};
use solana_program_runtime::loaded_programs::{
    LoadProgramMetrics, LoadedProgram, LoadedProgramsForTxBatch,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount},
    account_utils::StateMut,
    pubkey::Pubkey,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(crate) struct AccountsDb {
    inner: HashMap<Pubkey, AccountSharedData>,
    pub(crate) programs_cache: LoadedProgramsForTxBatch,
}

impl AccountsDb {
    pub(crate) fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.inner.get(pubkey).map(|acc| acc.to_owned())
    }

    pub(crate) fn add_account(&mut self, pubkey: Pubkey, data: AccountSharedData) {
        if data.executable() && pubkey != Pubkey::default() {
            let loaded_program = self.load_program(&data).unwrap();
            self.programs_cache
                .replenish(pubkey, Arc::new(loaded_program));
        }
        self.inner.insert(pubkey, data);
    }

    /// Skip the executable() checks for builtin accounts
    pub(crate) fn add_builtin_account(&mut self, pubkey: Pubkey, data: AccountSharedData) {
        self.inner.insert(pubkey, data);
    }

    pub(crate) fn sync_accounts(&mut self, mut accounts: Vec<(Pubkey, AccountSharedData)>) {
        // need to add programdata accounts first if there are any
        itertools::partition(&mut accounts, |x| {
            x.1.owner() == &bpf_loader_upgradeable::id()
                && x.1.data().first().map_or(false, |byte| *byte == 3)
        });
        for (pubkey, acc) in accounts {
            self.add_account(pubkey, acc);
        }
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
}
