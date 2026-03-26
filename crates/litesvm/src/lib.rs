/*!

# LiteSVM

## Overview

`litesvm` is a fast and lightweight library for testing Jupnet programs.
It works by creating an in-process Jupnet VM optimized for program developers.
This makes it much faster to run and compile than a full validator.
It has an ergonomic API with sane defaults and extensive configurability for those who want it.

### Minimal Example

```rust,no_run
use litesvm::LiteSVM;
use jupnet_sdk::{
    message::Message,
    pubkey::Pubkey,
    signer::{keypair::Keypair, Signer},
    system_instruction::transfer,
    transaction::Transaction,
};

let from_keypair = Keypair::new();
let from = from_keypair.pubkey();
let to = Pubkey::new_unique();

let mut svm = LiteSVM::new();
svm.airdrop(&from, 1_000_000_000).unwrap();
svm.airdrop(&to, 1_000_000_000).unwrap();

let instruction = transfer(&from, &to, 64);
let tx = Transaction::new(
    &[&from_keypair],
    Message::new(&[instruction], Some(&from)),
    svm.latest_blockhash(),
);
let tx_res = svm.send_transaction(tx).unwrap();

let from_account = svm.get_account(&from);
let to_account = svm.get_account(&to);
```

## Deploying Programs

Most of the time we want to do more than just mess around with token transfers -
we want to test our own programs.

To add a compiled program to our tests we can use [`.add_program_from_file`](LiteSVM::add_program_from_file).

Here's an example that deploys a program and sends a transaction to it:

```rust,no_run
use {
    litesvm::LiteSVM,
    jupnet_sdk::{
        instruction::{AccountMeta, Instruction},
        message::{Message, VersionedMessage},
        pubkey::Pubkey,
        signer::{keypair::Keypair, Signer},
        transaction::VersionedTransaction,
    },
};

fn test_logging() {
    let program_id = Pubkey::from_str_const("Logging111111111111111111111111111111111111");
    let account_meta = AccountMeta {
        pubkey: Pubkey::new_unique(),
        is_signer: false,
        is_writable: true,
    };
    let ix = Instruction {
        program_id,
        accounts: vec![account_meta],
        data: vec![5, 10, 11, 12, 13, 14],
    };
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let bytes = std::fs::read("path/to/program.so").unwrap();
    svm.add_program(program_id, &bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    let sim_res = svm.simulate_transaction(tx.clone()).unwrap();
    let meta = svm.send_transaction(tx).unwrap();
    assert_eq!(sim_res.meta, meta);
}

```

## Time travel

Many programs rely on the `Clock` sysvar: for example, a mint that doesn't become available until after
a certain time. With `litesvm` you can dynamically overwrite the `Clock` sysvar
using [`svm.set_sysvar::<Clock>()`](LiteSVM::set_sysvar).
Here's an example using a program that panics if `clock.unix_timestamp` is greater than 100
(which is on January 1st 1970):

```rust,no_run
use {
    litesvm::LiteSVM,
    jupnet_sdk::{
        clock::Clock,
        instruction::Instruction,
        message::{Message, VersionedMessage},
        pubkey::Pubkey,
        signer::{keypair::Keypair, Signer},
        sysvar::Sysvar,
        transaction::VersionedTransaction,
    },
};

fn test_set_clock() {
    let program_id = Pubkey::new_unique();
    let mut svm = LiteSVM::new();
    let bytes = std::fs::read("path/to/clock_example.so").unwrap();
    svm.add_program(program_id, &bytes).unwrap();
    let payer = Keypair::new();
    let payer_address = payer.pubkey();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let blockhash = svm.latest_blockhash();
    let ixs = [Instruction {
        program_id,
        data: vec![],
        accounts: vec![],
    }];
    let msg = Message::new_with_blockhash(&ixs, Some(&payer_address), &blockhash);
    let versioned_msg = VersionedMessage::Legacy(msg);
    let tx = VersionedTransaction::try_new(versioned_msg, &[&payer]).unwrap();
    // set the time to January 1st 2000
    let mut initial_clock = svm.get_sysvar::<Clock>();
    initial_clock.unix_timestamp = 1735689600;
    svm.set_sysvar::<Clock>(&initial_clock);
    // this will fail because it's not January 1970 anymore
    svm.send_transaction(tx).unwrap_err();
    // so let's turn back time
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = 50;
    svm.set_sysvar::<Clock>(&clock);
    let ixs2 = [Instruction {
        program_id,
        data: vec![1],
        accounts: vec![],
    }];
    let msg2 = Message::new_with_blockhash(&ixs2, Some(&payer_address), &blockhash);
    let versioned_msg2 = VersionedMessage::Legacy(msg2);
    let tx2 = VersionedTransaction::try_new(versioned_msg2, &[&payer]).unwrap();
    // now the transaction goes through
    svm.send_transaction(tx2).unwrap();
}

```

See also: [`warp_to_slot`](LiteSVM::warp_to_slot), which lets you jump to a future slot.

## Writing arbitrary accounts

LiteSVM lets you write any account data you want, regardless of
whether the account state would even be possible.

```rust,no_run
use {
    litesvm::LiteSVM,
    jupnet_sdk::{
        account::Account,
        pubkey::Pubkey,
    },
};

fn test_write_account() {
    let owner = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let mut svm = LiteSVM::new();
    svm.set_account(
        account_key,
        Account {
            lamports: 1_000_000_000,
            data: vec![1, 2, 3, 4],
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let raw_account = svm.get_account(&account_key).unwrap();
    assert_eq!(raw_account.data, vec![1, 2, 3, 4]);
    assert_eq!(raw_account.owner, owner);
}

```

## Register tracing

`litesvm` can be instantiated with the capability to provide register tracing
data from processed transactions. This functionality is gated behind the
`register-tracing` feature flag, which in turn relies on the
`invocation-inspect-callback` flag. To enable it, users can either
construct `litesvm` with the `LiteSVM::new_debuggable` initializer - allowing
register tracing to be configured directly - or simply set the `SBF_TRACE_DIR`
environment variable, which `litesvm` interprets as a signal to turn tracing on
upon instantiation. The latter allows users to take advantage of the
functionality without actually doing any changes to their code.

A default post-instruction callback is provided for storing the
register tracing data in files. It persists the register sets,
the SBPF instructions, and a SHA-256 hash identifying the executable that
was used to generate the tracing data. If the `SBF_TRACE_DISASSEMBLE`
environment variable is set, a disassembled register trace will also be
produced for each collected register trace. The motivation behind providing the
SHA-256 identifier is that files may grow in number, and consumers need a
deterministic way to evaluate which shared object should be used when
analyzing the tracing data.

Once enabled register tracing can't be changed afterwards because in nature
it's baked into the program executables at load time. Yet a user may want a
more fine-grained control over when register tracing data should be
collected - for example, only for a specific instruction. Such control could
be achieved by resetting the invocation callback to
`EmptyInvocationInspectCallback` and later by restoring it to
`DefaultRegisterTracingCallback`.

## Other features

Other things you can do with `litesvm` include:

* Changing the max compute units and other compute budget behaviour using [`.with_compute_budget`](LiteSVM::with_compute_budget).
* Disable transaction signature checking using [`.with_sigverify(false)`](LiteSVM::with_sigverify).
* Find previous transactions using [`.get_transaction`](`LiteSVM::get_transaction`).

*/

#[allow(deprecated)]
use jupnet_sdk::sysvar::recent_blockhashes::IterItem;
#[allow(deprecated)]
use jupnet_sdk::sysvar::{fees::Fees, recent_blockhashes::RecentBlockhashes};
#[cfg(feature = "nodejs-internal")]
use qualifier_attr::qualifiers;
use {
    crate::{
        accounts_db::AccountsDb,
        error::LiteSVMError,
        history::TransactionHistory,
        message_processor::process_message,
        programs::load_default_programs,
        types::{
            ExecutionResult, FailedTransactionMetadata, TransactionMetadata, TransactionResult,
        },
        utils::{
            builtins::BUILTINS,
            create_blockhash,
            rent::{check_rent_state_with_account, get_account_rent_state, RentState},
        },
    },
    jupnet_bpf_loader_program::{
        load_program_from_bytes,
        syscalls::{create_program_runtime_environment_v1, create_program_runtime_environment_v2},
    },
    jupnet_compute_budget::{
        compute_budget::ComputeBudget, compute_budget_limits::ComputeBudgetLimits,
    },
    jupnet_log_collector::LogCollector,
    jupnet_program_runtime::{
        invoke_context::{BuiltinFunctionWithContext, EnvironmentConfig, InvokeContext},
        jupnet_rbpf::program::BuiltinFunction,
        loaded_programs::{LoadProgramMetrics, ProgramCacheEntry},
    },
    jupnet_sdk::{
        account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
        bpf_loader, bpf_loader_deprecated,
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        clock::Clock,
        epoch_rewards::EpochRewards,
        epoch_schedule::EpochSchedule,
        feature::{self, Feature},
        feature_set::FeatureSet,
        fee::FeeStructure,
        hash::Hash,
        inner_instruction::InnerInstructionsList,
        message::{Message, VersionedMessage},
        native_loader,
        native_token::MOTES_PER_JUP,
        nonce::state::DurableNonce,
        pubkey::Pubkey,
        rent::Rent,
        reserved_account_keys::ReservedAccountKeys,
        signature::TypedSignature,
        signer::{keypair::Keypair, Signer},
        slot_hashes::SlotHashes,
        slot_history::SlotHistory,
        stake_history::StakeHistory,
        system_program,
        sysvar::{last_restart_slot::LastRestartSlot, Sysvar, SysvarId},
        transaction::{MessageHash, SanitizedTransaction, VersionedTransaction},
        transaction_context::{ExecutionRecord, IndexOfAccount, TransactionContext},
    },
    jupnet_svm_transaction::svm_message::{SVMExecutionMessage, SVMMessage},
    jupnet_system_program::{get_system_account_kind, SystemAccountKind},
    jupnet_timings::ExecuteTimings,
    jupnet_transaction_error::TransactionError,
    log::error,
    serde::de::DeserializeOwned,
    std::{cell::RefCell, path::Path, rc::Rc, sync::Arc},
    types::SimulatedTransactionInfo,
    utils::{
        construct_instructions_account,
        inner_instructions::inner_instructions_list_from_instruction_trace,
    },
};

pub mod error;
pub mod types;

type TransactionAccount = (Pubkey, AccountSharedData);
type TransactionProgramIndices = Vec<Vec<IndexOfAccount>>;

mod accounts_db;
mod format_logs;
mod history;
mod message_processor;
mod programs;
mod utils;

#[derive(Clone)]
pub struct LiteSVM {
    accounts: AccountsDb,
    airdrop_kp: [u8; 64],
    feature_set: FeatureSet,
    reserved_account_keys: ReservedAccountKeys,
    latest_blockhash: Hash,
    history: TransactionHistory,
    compute_budget: Option<ComputeBudget>,
    sigverify: bool,
    blockhash_check: bool,
    fee_structure: FeeStructure,
    log_bytes_limit: Option<usize>,
}

impl Default for LiteSVM {
    fn default() -> Self {
        let _enable_register_tracing = false;

        Self::new_inner(_enable_register_tracing)
    }
}

impl LiteSVM {
    fn new_inner(_enable_register_tracing: bool) -> Self {
        let feature_set = FeatureSet::default();

        #[allow(unused_mut)]
        let mut svm = Self {
            accounts: Default::default(),
            airdrop_kp: Keypair::new().to_bytes(),
            reserved_account_keys: Self::reserved_account_keys_for_feature_set(&feature_set),
            feature_set,
            latest_blockhash: create_blockhash(b"genesis"),
            history: TransactionHistory::new(),
            compute_budget: None,
            sigverify: false,
            blockhash_check: false,
            fee_structure: FeeStructure::default(),
            log_bytes_limit: Some(10_000),
        };

        svm
    }

    fn into_basic(self) -> Self {
        self.with_feature_set(FeatureSet::all_enabled())
            .with_builtins()
            .with_lamports(1_000_000u64.wrapping_mul(MOTES_PER_JUP))
            .with_sysvars()
            .with_feature_accounts()
            .with_default_programs()
            .with_sigverify(true)
            .with_blockhash_check(true)
    }

    /// Creates the basic test environment.
    pub fn new() -> Self {
        LiteSVM::default().into_basic()
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_compute_budget(&mut self, compute_budget: ComputeBudget) {
        self.compute_budget = Some(compute_budget);
    }

    /// Sets the compute budget.
    pub fn with_compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        self.set_compute_budget(compute_budget);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_sigverify(&mut self, sigverify: bool) {
        self.sigverify = sigverify;
    }

    /// Enables or disables sigverify.
    pub fn with_sigverify(mut self, sigverify: bool) -> Self {
        self.set_sigverify(sigverify);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_blockhash_check(&mut self, check: bool) {
        self.blockhash_check = check;
    }

    /// Enables or disables the blockhash check.
    pub fn with_blockhash_check(mut self, check: bool) -> Self {
        self.set_blockhash_check(check);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_sysvars(&mut self) {
        self.set_sysvar(&Clock::default());
        self.set_sysvar(&EpochRewards::default());
        self.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        let fees = Fees::default();
        self.set_sysvar(&fees);
        self.set_sysvar(&LastRestartSlot::default());
        let latest_blockhash = self.latest_blockhash;
        #[allow(deprecated)]
        self.set_sysvar(&RecentBlockhashes::from_iter([IterItem(
            0,
            &latest_blockhash,
            fees.fee_calculator.lamports_per_signature,
        )]));

        self.set_sysvar(&Rent::default());
        self.set_sysvar(&SlotHashes::new(&[(
            self.accounts.sysvar_cache.get_clock().unwrap().slot,
            latest_blockhash,
        )]));
        self.set_sysvar(&SlotHistory::default());
        self.set_sysvar(&StakeHistory::default());
    }

    /// Includes the default sysvars.
    pub fn with_sysvars(mut self) -> Self {
        self.set_sysvars();
        self
    }

    /// Set the FeatureSet used by the VM instance.
    pub fn with_feature_set(mut self, feature_set: FeatureSet) -> Self {
        self.set_feature_set(feature_set);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_feature_set(&mut self, feature_set: FeatureSet) {
        self.feature_set = feature_set;
        self.reserved_account_keys = Self::reserved_account_keys_for_feature_set(&self.feature_set);
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_feature_accounts(&mut self) {
        for (feature_id, activation_slot) in &self.feature_set.active {
            let feature_account = Feature {
                activated_at: Some(*activation_slot),
            };
            let lamports = self.minimum_balance_for_rent_exemption(Feature::size_of());
            let data_len = Feature::size_of()
                .max(bincode::serialized_size(&feature_account).unwrap() as usize);
            let mut account = AccountSharedData::new(lamports, data_len, &feature::id());
            bincode::serialize_into(account.data_as_mut_slice(), &feature_account).ok();
            self.accounts.add_account_no_checks(*feature_id, account);
        }
    }

    pub fn with_feature_accounts(mut self) -> Self {
        self.set_feature_accounts();
        self
    }

    fn reserved_account_keys_for_feature_set(feature_set: &FeatureSet) -> ReservedAccountKeys {
        let mut reserved_account_keys = ReservedAccountKeys::default();
        reserved_account_keys.update_active_set(feature_set);
        reserved_account_keys
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_builtins(&mut self) {
        BUILTINS.iter().for_each(|builtint| {
            if builtint
                .enable_feature_id
                .is_none_or(|x| self.feature_set.is_active(&x))
            {
                let loaded_program =
                    ProgramCacheEntry::new_builtin(0, builtint.name.len(), builtint.entrypoint);
                self.accounts
                    .programs_cache
                    .replenish(builtint.program_id, Arc::new(loaded_program));
                self.accounts.add_builtin_account(
                    builtint.program_id,
                    crate::utils::create_loadable_account_for_test(builtint.name),
                );
            }
        });

        let _enable_register_tracing = false;

        let compute_budget = self.compute_budget.unwrap_or_default();
        let program_runtime_v1 = create_program_runtime_environment_v1(
            &self.feature_set,
            &compute_budget,
            false,
            _enable_register_tracing,
        )
        .unwrap();

        let program_runtime_v2 =
            create_program_runtime_environment_v2(&compute_budget, _enable_register_tracing);

        self.accounts.environments.program_runtime_v1 = Arc::new(program_runtime_v1);
        self.accounts.environments.program_runtime_v2 = Arc::new(program_runtime_v2);
    }

    /// Changes the default builtins.
    //
    // Use `with_feature_set` beforehand to change change what builtins are added.
    pub fn with_builtins(mut self) -> Self {
        self.set_builtins();
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_lamports(&mut self, lamports: u64) {
        self.accounts.add_account_no_checks(
            Keypair::from_bytes(self.airdrop_kp.as_slice())
                .unwrap()
                .pubkey(),
            AccountSharedData::new(lamports, 0, &system_program::id()),
        );
    }

    /// Changes the initial lamports in LiteSVM's airdrop account.
    pub fn with_lamports(mut self, lamports: u64) -> Self {
        self.set_lamports(lamports);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_default_programs(&mut self) {
        load_default_programs(self);
    }

    /// Includes the standard SPL programs.
    pub fn with_default_programs(mut self) -> Self {
        self.set_default_programs();
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_transaction_history(&mut self, capacity: usize) {
        self.history.set_capacity(capacity);
    }

    /// Changes the capacity of the transaction history.
    /// Set this to 0 to disable transaction history and allow duplicate transactions.
    pub fn with_transaction_history(mut self, capacity: usize) -> Self {
        self.set_transaction_history(capacity);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_log_bytes_limit(&mut self, limit: Option<usize>) {
        self.log_bytes_limit = limit;
    }

    pub fn with_log_bytes_limit(mut self, limit: Option<usize>) -> Self {
        self.set_log_bytes_limit(limit);
        self
    }

    /// Returns minimum balance required to make an account with specified data length rent exempt.
    pub fn minimum_balance_for_rent_exemption(&self, data_len: usize) -> u64 {
        1.max(
            self.accounts
                .sysvar_cache
                .get_rent()
                .unwrap_or_default()
                .minimum_balance(data_len),
        )
    }

    /// Returns all information associated with the account of the provided pubkey.
    pub fn get_account(&self, address: &Pubkey) -> Option<Account> {
        self.accounts.get_account(address).map(Into::into)
    }

    /// Sets all information associated with the account of the provided pubkey.
    pub fn set_account(&mut self, address: Pubkey, data: Account) -> Result<(), LiteSVMError> {
        self.accounts.add_account(address, data.into())
    }

    /// **⚠️ ADVANCED USE ONLY ⚠️**
    ///
    /// Returns a reference to the internal accounts database.
    ///
    /// This provides read-only access to the accounts database for advanced inspection.
    /// Use [`get_account`](LiteSVM::get_account) for normal account retrieval.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use litesvm::LiteSVM;
    ///
    /// let svm = LiteSVM::new();
    ///
    /// // Read-only access to accounts database
    /// let accounts_db = svm.accounts_db();
    /// // ... inspect internal state if needed
    /// ```
    pub fn accounts_db(&self) -> &AccountsDb {
        &self.accounts
    }

    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&self, address: &Pubkey) -> Option<u64> {
        self.accounts.get_account_ref(address).map(|x| x.lamports())
    }

    /// Gets the latest blockhash.
    pub fn latest_blockhash(&self) -> Hash {
        self.latest_blockhash
    }

    /// Sets the sysvar to the test environment.
    pub fn set_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        let mut account = AccountSharedData::new(1, T::size_of(), &jupnet_sdk::sysvar::id());
        account.serialize_data(sysvar).unwrap();
        self.accounts.add_account(T::id(), account).unwrap();
    }

    /// Gets a sysvar from the test environment.
    pub fn get_sysvar<T>(&self) -> T
    where
        T: Sysvar + SysvarId + DeserializeOwned,
    {
        bincode::deserialize(self.accounts.get_account_ref(&T::id()).unwrap().data()).unwrap()
    }

    /// Gets a transaction from the transaction history.
    pub fn get_transaction(&self, signature: &TypedSignature) -> Option<&TransactionResult> {
        self.history.get_transaction(signature)
    }

    /// Returns the pubkey of the internal airdrop account.
    pub fn airdrop_pubkey(&self) -> Pubkey {
        Keypair::from_bytes(self.airdrop_kp.as_slice())
            .unwrap()
            .pubkey()
    }

    /// Airdrops the account with the lamports specified.
    pub fn airdrop(&mut self, address: &Pubkey, lamports: u64) -> TransactionResult {
        let payer = Keypair::from_bytes(self.airdrop_kp.as_slice()).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(Message::new_with_blockhash(
                &[jupnet_sdk::system_instruction::transfer(
                    &payer.pubkey(),
                    address,
                    lamports,
                )],
                Some(&payer.pubkey()),
                &self.latest_blockhash,
            )),
            &[payer],
        )
        .unwrap();

        self.send_transaction(tx)
    }

    /// Adds a builtin program to the test environment.
    pub fn add_builtin(&mut self, program_id: Pubkey, entrypoint: BuiltinFunctionWithContext) {
        let builtin = ProgramCacheEntry::new_builtin(
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

        let mut account = AccountSharedData::new(1, 1, &bpf_loader::id());
        account.set_executable(true);
        self.accounts.add_account_no_checks(program_id, account);
    }

    /// Adds an SBF program to the test environment from the file specified.
    pub fn add_program_from_file(
        &mut self,
        program_id: impl Into<Pubkey>,
        path: impl AsRef<Path>,
    ) -> Result<(), LiteSVMError> {
        let bytes = std::fs::read(path)?;
        self.add_program(program_id, &bytes)?;
        Ok(())
    }

    fn add_program_internal<const PREVERIFIED: bool>(
        &mut self,
        program_id: impl Into<Pubkey>,
        program_bytes: &[u8],
        loader_id: &Pubkey,
    ) -> Result<(), LiteSVMError> {
        let program_id = program_id.into();
        let current_slot = self
            .accounts
            .sysvar_cache
            .get_clock()
            .unwrap_or_default()
            .slot;

        let program_size = if bpf_loader_upgradeable::check_id(loader_id) {
            let (programdata_address, _bump) =
                Pubkey::find_program_address(&[program_id.as_ref()], loader_id);

            let programdata_metadata_len = UpgradeableLoaderState::size_of_programdata_metadata();
            let programdata_len = programdata_metadata_len + program_bytes.len();
            let mut programdata_data = vec![0u8; programdata_len];

            bincode::serialize_into(
                &mut programdata_data[..programdata_metadata_len],
                &UpgradeableLoaderState::ProgramData {
                    slot: current_slot,
                    upgrade_authority_address: None,
                },
            )
            .expect("UpgradeableLoaderState::ProgramData serialization should never fail");
            programdata_data[programdata_metadata_len..].copy_from_slice(program_bytes);

            let programdata_lamports = self.minimum_balance_for_rent_exemption(programdata_len);
            let mut programdata_account =
                AccountSharedData::new(programdata_lamports, programdata_len, loader_id);
            programdata_account.set_data_from_slice(&programdata_data);

            let program_account_data = bincode::serialize(&UpgradeableLoaderState::Program {
                programdata_address,
            })
            .expect("UpgradeableLoaderState::Program serialization should never fail");

            let program_lamports =
                self.minimum_balance_for_rent_exemption(program_account_data.len());
            let mut program_account =
                AccountSharedData::new(program_lamports, program_account_data.len(), loader_id);
            program_account.set_executable(true);
            program_account.set_data_from_slice(&program_account_data);

            self.accounts
                .add_account_no_checks(programdata_address, programdata_account);
            self.accounts
                .add_account_no_checks(program_id, program_account);

            programdata_len
        } else if bpf_loader::check_id(loader_id) || bpf_loader_deprecated::check_id(loader_id) {
            let program_len = program_bytes.len();
            let lamports = self.minimum_balance_for_rent_exemption(program_len);
            let mut account = AccountSharedData::new(lamports, program_len, loader_id);
            account.set_executable(true);
            account.set_data_from_slice(program_bytes);

            self.accounts.add_account_no_checks(program_id, account);

            program_len
        } else {
            return Err(LiteSVMError::InvalidLoader(format!(
                "Unsupported loader: {loader_id}"
            )));
        };

        let mut loaded_program = load_program_from_bytes(
            None,
            &mut LoadProgramMetrics::default(),
            program_bytes,
            loader_id,
            program_size,
            current_slot,
            self.accounts.environments.program_runtime_v1.clone(),
            PREVERIFIED,
        )
        .map_err(LiteSVMError::from)?;
        loaded_program.effective_slot = current_slot;

        self.accounts
            .programs_cache
            .replenish(program_id, Arc::new(loaded_program));

        Ok(())
    }

    /// Adds an SBF program to the test environment.
    ///
    /// Uses `BPFLoaderUpgradeable` by default for the loader.
    pub fn add_program(
        &mut self,
        program_id: impl Into<Pubkey>,
        program_bytes: &[u8],
    ) -> Result<(), LiteSVMError> {
        self.add_program_internal::<false>(program_id, program_bytes, &bpf_loader_upgradeable::id())
    }

    /// Adds an SBF program with a specific loader to match mainnet CU behavior.
    ///
    /// Use `bpf_loader::id()` for BPFLoader2, `bpf_loader_deprecated::id()` for BPFLoader1,
    /// or `bpf_loader_upgradeable::id()` for the upgradeable loader.
    pub fn add_program_with_loader(
        &mut self,
        program_id: impl Into<Pubkey>,
        program_bytes: &[u8],
        loader_id: Pubkey,
    ) -> Result<(), LiteSVMError> {
        self.add_program_internal::<false>(program_id, program_bytes, &loader_id)
    }

    /// Adds an SBF program that is known-good and already verified.
    pub(crate) fn add_program_preverified(
        &mut self,
        program_id: impl Into<Pubkey>,
        program_bytes: &[u8],
        loader_id: &Pubkey,
    ) -> Result<(), LiteSVMError> {
        self.add_program_internal::<true>(program_id, program_bytes, loader_id)
    }

    fn sanitize_transaction_no_verify_inner(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        let res = SanitizedTransaction::try_create(
            tx,
            MessageHash::Compute,
            Some(false),
            &self.reserved_account_keys.active,
        );
        res.inspect_err(|_| {
            log::error!("Transaction sanitization failed");
        })
    }

    fn sanitize_transaction_no_verify(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, ExecutionResult> {
        self.sanitize_transaction_no_verify_inner(tx)
            .map_err(|err| ExecutionResult {
                tx_result: Err(err),
                ..Default::default()
            })
    }

    fn sanitize_transaction(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, ExecutionResult> {
        self.sanitize_transaction_inner(tx)
            .map_err(|err| ExecutionResult {
                tx_result: Err(err),
                ..Default::default()
            })
    }

    fn sanitize_transaction_inner(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        let tx = self.sanitize_transaction_no_verify_inner(tx)?;

        tx.verify()?;
        SanitizedTransaction::validate_account_locks(tx.message(), 10, 128, 4096)?;

        Ok(tx)
    }

    fn process_transaction(
        &self,
        tx: &SanitizedTransaction,
        compute_budget_limits: ComputeBudgetLimits,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> (
        Result<(), TransactionError>,
        u64,
        Option<TransactionContext>,
        u64,
        Option<Pubkey>,
    ) {
        let message = tx.message();
        let account_keys = message.account_keys();

        let mut transaction_accounts: Vec<(Pubkey, AccountSharedData)> = account_keys
            .iter()
            .map(|key| {
                let account = self.accounts.get_account(key).unwrap_or_default();
                (*key, account)
            })
            .collect();

        let base_fee = self
            .fee_structure
            .lamports_per_signature
            .saturating_mul(SVMMessage::num_fee_paying_signatures(tx));
        let prioritization_fee = compute_budget_limits
            .compute_unit_price
            .saturating_mul(u64::from(compute_budget_limits.compute_unit_limit))
            .saturating_div(1_000_000);
        let fee = base_fee.saturating_add(prioritization_fee);

        let payer_address = *message.fee_payer();
        let rent: Rent = self.get_sysvar();

        if let Err(err) = validate_fee_payer(
            &payer_address,
            &mut transaction_accounts[0].1,
            0,
            &rent,
            fee,
        ) {
            return (Err(err), 0, None, fee, Some(payer_address));
        }

        let compute_budget = self
            .compute_budget
            .unwrap_or_else(|| ComputeBudget::from(compute_budget_limits));

        let has_instructions_sysvar_account_index = transaction_accounts
            .iter()
            .position(|(pk, _)| jupnet_sdk::sysvar::instructions::check_id(pk));
        if let Some(idx) = has_instructions_sysvar_account_index {
            transaction_accounts[idx].1 = construct_instructions_account(message);
        }

        let execution_messages = SVMMessage::get_execution_messages(tx);

        if !SVMMessage::is_batched_message(tx) {
            self.process_single_message(
                tx,
                &execution_messages[0],
                transaction_accounts,
                compute_budget,
                log_collector,
                &rent,
                fee,
                payer_address,
            )
        } else {
            self.process_batched_messages(
                tx,
                &execution_messages,
                transaction_accounts,
                compute_budget,
                log_collector,
                &rent,
                fee,
                payer_address,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_single_message(
        &self,
        tx: &SanitizedTransaction,
        exec_message: &SVMExecutionMessage,
        mut transaction_accounts: Vec<(Pubkey, AccountSharedData)>,
        compute_budget: ComputeBudget,
        log_collector: Rc<RefCell<LogCollector>>,
        rent: &Rent,
        fee: u64,
        payer_address: Pubkey,
    ) -> (
        Result<(), TransactionError>,
        u64,
        Option<TransactionContext>,
        u64,
        Option<Pubkey>,
    ) {
        let program_indices = match calculate_program_indices(
            exec_message,
            &mut transaction_accounts,
            &self.accounts,
        ) {
            Ok(indices) => indices,
            Err(e) => return (Err(e), 0, None, fee, Some(payer_address)),
        };

        let mut transaction_context = TransactionContext::new(
            transaction_accounts,
            rent.clone(),
            compute_budget.max_instruction_stack_depth,
            compute_budget.max_instruction_trace_length,
        );
        #[cfg(debug_assertions)]
        transaction_context.set_signature(&tx.signature().get_signature());

        let mut programs_cache = self.accounts.programs_cache.clone();
        let mut executed_units = 0u64;

        let mut invoke_context = InvokeContext::new(
            &mut transaction_context,
            &mut programs_cache,
            EnvironmentConfig::new(
                self.latest_blockhash,
                None,
                None,
                None,
                Arc::new(self.feature_set.clone()),
                self.fee_structure.lamports_per_signature,
                &self.accounts.sysvar_cache,
            ),
            Some(log_collector),
            compute_budget,
        );
        let result = process_message(
            exec_message,
            &program_indices,
            &mut invoke_context,
            &mut ExecuteTimings::default(),
            &mut executed_units,
        );
        drop(invoke_context);

        let result = result.and_then(|_| self.check_accounts_rent(tx, &transaction_context, rent));

        (
            result,
            executed_units,
            Some(transaction_context),
            fee,
            Some(payer_address),
        )
    }

    /// Processes a batched transaction containing multiple execution steps.
    ///
    /// Each step is executed in isolation: successful steps propagate their
    /// account changes to the next step, while failed steps are discarded
    /// without affecting subsequent steps.
    #[allow(clippy::too_many_arguments)]
    fn process_batched_messages(
        &self,
        tx: &SanitizedTransaction,
        execution_messages: &[SVMExecutionMessage],
        mut transaction_accounts: Vec<(Pubkey, AccountSharedData)>,
        compute_budget: ComputeBudget,
        log_collector: Rc<RefCell<LogCollector>>,
        rent: &Rent,
        fee: u64,
        payer_address: Pubkey,
    ) -> (
        Result<(), TransactionError>,
        u64,
        Option<TransactionContext>,
        u64,
        Option<Pubkey>,
    ) {
        let signatures = tx.signatures();
        let mut total_executed_units = 0u64;

        for exec_message in execution_messages {
            let step_budget = match exec_message.compute_unit_limit {
                Some(limit) => {
                    let mut budget = compute_budget;
                    budget.compute_unit_limit = u64::from(limit);
                    budget
                }
                None => compute_budget,
            };

            let mut step_accounts = transaction_accounts.clone();
            let program_indices =
                match calculate_program_indices(exec_message, &mut step_accounts, &self.accounts) {
                    Ok(indices) => indices,
                    Err(e) => {
                        return (Err(e), total_executed_units, None, fee, Some(payer_address))
                    }
                };

            let mut step_context = TransactionContext::new(
                step_accounts,
                rent.clone(),
                step_budget.max_instruction_stack_depth,
                step_budget.max_instruction_trace_length,
            );
            #[cfg(debug_assertions)]
            step_context.set_signature(&signatures[exec_message.signature_index].get_signature());

            let mut programs_cache = self.accounts.programs_cache.clone();
            let mut executed_units = 0u64;

            let mut invoke_context = InvokeContext::new(
                &mut step_context,
                &mut programs_cache,
                EnvironmentConfig::new(
                    self.latest_blockhash,
                    None,
                    None,
                    None,
                    Arc::new(self.feature_set.clone()),
                    self.fee_structure.lamports_per_signature,
                    &self.accounts.sysvar_cache,
                ),
                Some(Rc::clone(&log_collector)),
                step_budget,
            );
            let step_result = process_message(
                exec_message,
                &program_indices,
                &mut invoke_context,
                &mut ExecuteTimings::default(),
                &mut executed_units,
            );
            drop(invoke_context);

            total_executed_units += executed_units;

            let step_result =
                step_result.and_then(|_| self.check_accounts_rent(tx, &step_context, rent));

            if step_result.is_ok() {
                let ExecutionRecord { accounts, .. } = step_context.into();
                transaction_accounts = accounts;
            }
        }

        let final_context = TransactionContext::new(
            transaction_accounts,
            rent.clone(),
            compute_budget.max_instruction_stack_depth,
            compute_budget.max_instruction_trace_length,
        );

        (
            Ok(()),
            total_executed_units,
            Some(final_context),
            fee,
            Some(payer_address),
        )
    }

    fn check_accounts_rent(
        &self,
        tx: &SanitizedTransaction,
        context: &TransactionContext,
        rent: &Rent,
    ) -> Result<(), TransactionError> {
        let message = tx.message();
        for index in 0..message.account_keys().len() {
            if message.is_writable(index) {
                let account = context
                    .accounts()
                    .try_borrow(index as IndexOfAccount)
                    .map_err(|err| TransactionError::InstructionError(index as u8, err))?;

                let pubkey = context
                    .get_key_of_account_at_index(index as IndexOfAccount)
                    .map_err(|err| TransactionError::InstructionError(index as u8, err))?;

                let post_rent_state =
                    get_account_rent_state(rent, account.lamports(), account.data().len());
                let pre_rent_state = self
                    .accounts
                    .get_account_ref(pubkey)
                    .map(|acc| get_account_rent_state(rent, acc.lamports(), acc.data().len()))
                    .unwrap_or(RentState::Uninitialized);

                check_rent_state_with_account(
                    &pre_rent_state,
                    &post_rent_state,
                    pubkey,
                    index as IndexOfAccount,
                )?;
            }
        }
        Ok(())
    }

    fn execute_transaction_no_verify(
        &mut self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction_no_verify(tx), |s_tx| {
            self.execute_sanitized_transaction(&s_tx, log_collector)
        })
    }

    fn execute_transaction(
        &mut self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction(tx), |s_tx| {
            self.execute_sanitized_transaction(&s_tx, log_collector)
        })
    }

    fn execute_sanitized_transaction(
        &mut self,
        sanitized_tx: &SanitizedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        let CheckAndProcessTransactionSuccess {
            core:
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                },
            fee,
            payer_key,
        } = match self.check_and_process_transaction(sanitized_tx, log_collector) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            let mut exec_result =
                execution_result_if_context(sanitized_tx, ctx, result, compute_units_consumed, fee);

            if let Some(payer) = payer_key.filter(|_| exec_result.tx_result.is_err()) {
                exec_result.tx_result = self
                    .accounts
                    .withdraw(&payer, fee)
                    .and(exec_result.tx_result);
            }
            exec_result
        } else {
            ExecutionResult {
                tx_result: result,
                compute_units_consumed,
                fee,
                ..Default::default()
            }
        }
    }

    fn execute_sanitized_transaction_readonly(
        &self,
        sanitized_tx: &SanitizedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        let CheckAndProcessTransactionSuccess {
            core:
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                },
            fee,
            ..
        } = match self.check_and_process_transaction(sanitized_tx, log_collector) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            execution_result_if_context(sanitized_tx, ctx, result, compute_units_consumed, fee)
        } else {
            ExecutionResult {
                tx_result: result,
                compute_units_consumed,
                fee,
                ..Default::default()
            }
        }
    }

    fn check_and_process_transaction<'a, 'b>(
        &'a self,
        sanitized_tx: &'b SanitizedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> Result<CheckAndProcessTransactionSuccess, ExecutionResult>
    where
        'a: 'b,
    {
        self.maybe_blockhash_check(sanitized_tx)?;
        let compute_budget_limits = get_compute_budget_limits(sanitized_tx, &self.feature_set)?;
        self.maybe_history_check(sanitized_tx)?;
        let (result, compute_units_consumed, context, fee, payer_key) =
            self.process_transaction(sanitized_tx, compute_budget_limits, log_collector);
        Ok(CheckAndProcessTransactionSuccess {
            core: {
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                }
            },
            fee,
            payer_key,
        })
    }

    fn maybe_history_check(
        &self,
        sanitized_tx: &SanitizedTransaction,
    ) -> Result<(), ExecutionResult> {
        if self.sigverify && self.history.check_transaction(sanitized_tx.signature()) {
            return Err(ExecutionResult {
                tx_result: Err(TransactionError::AlreadyProcessed),
                ..Default::default()
            });
        }
        Ok(())
    }

    fn maybe_blockhash_check(
        &self,
        sanitized_tx: &SanitizedTransaction,
    ) -> Result<(), ExecutionResult> {
        if self.blockhash_check {
            self.check_transaction_age(sanitized_tx)?;
        }
        Ok(())
    }

    fn execute_transaction_readonly(
        &self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction(tx), |s_tx| {
            self.execute_sanitized_transaction_readonly(&s_tx, log_collector)
        })
    }

    fn execute_transaction_no_verify_readonly(
        &self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction_no_verify(tx), |s_tx| {
            self.execute_sanitized_transaction_readonly(&s_tx, log_collector)
        })
    }

    /// Submits a signed transaction.
    pub fn send_transaction(&mut self, tx: impl Into<VersionedTransaction>) -> TransactionResult {
        let log_collector = LogCollector {
            bytes_limit: self.log_bytes_limit,
            ..Default::default()
        };
        let log_collector = Rc::new(RefCell::new(log_collector));
        let vtx: VersionedTransaction = tx.into();
        let ExecutionResult {
            post_accounts,
            tx_result,
            signature,
            compute_units_consumed,
            inner_instructions,
            return_data,
            included,
            fee,
        } = if self.sigverify {
            self.execute_transaction(vtx, log_collector.clone())
        } else {
            self.execute_transaction_no_verify(vtx, log_collector.clone())
        };
        let Ok(logs) = Rc::try_unwrap(log_collector).map(|lc| lc.into_inner().messages) else {
            unreachable!("Log collector should not be used after send_transaction returns")
        };
        let meta = TransactionMetadata {
            logs,
            inner_instructions,
            compute_units_consumed,
            return_data,
            signature: signature.clone(),
            fee,
        };

        if let Err(tx_err) = tx_result {
            let err = TransactionResult::Err(FailedTransactionMetadata { err: tx_err, meta });
            if included {
                self.history.add_new_transaction(signature, err.clone());
            }
            err
        } else {
            self.history
                .add_new_transaction(signature, Ok(meta.clone()));
            self.accounts
                .sync_accounts(post_accounts)
                .expect("It shouldn't be possible to write invalid sysvars in send_transaction.");

            TransactionResult::Ok(meta)
        }
    }

    /// Simulates a transaction.
    pub fn simulate_transaction(
        &self,
        tx: impl Into<VersionedTransaction>,
    ) -> Result<SimulatedTransactionInfo, FailedTransactionMetadata> {
        let log_collector = LogCollector {
            bytes_limit: self.log_bytes_limit,
            ..Default::default()
        };
        let log_collector = Rc::new(RefCell::new(log_collector));
        let ExecutionResult {
            post_accounts,
            tx_result,
            signature,
            compute_units_consumed,
            inner_instructions,
            return_data,
            fee,
            ..
        } = if self.sigverify {
            self.execute_transaction_readonly(tx.into(), log_collector.clone())
        } else {
            self.execute_transaction_no_verify_readonly(tx.into(), log_collector.clone())
        };
        let Ok(logs) = Rc::try_unwrap(log_collector).map(|lc| lc.into_inner().messages) else {
            unreachable!("Log collector should not be used after simulate_transaction returns")
        };
        let meta = TransactionMetadata {
            signature,
            logs,
            inner_instructions,
            compute_units_consumed,
            return_data,
            fee,
        };

        if let Err(tx_err) = tx_result {
            Err(FailedTransactionMetadata { err: tx_err, meta })
        } else {
            Ok(SimulatedTransactionInfo {
                meta,
                post_accounts,
            })
        }
    }

    /// Expires the current blockhash.
    pub fn expire_blockhash(&mut self) {
        self.latest_blockhash = create_blockhash(&self.latest_blockhash.to_bytes());
        #[allow(deprecated)]
        self.set_sysvar(&RecentBlockhashes::from_iter([IterItem(
            0,
            &self.latest_blockhash,
            self.fee_structure.lamports_per_signature,
        )]));
    }

    /// Warps the clock to the specified slot.
    pub fn warp_to_slot(&mut self, slot: u64) {
        let mut clock = self.get_sysvar::<Clock>();
        clock.slot = slot;
        self.set_sysvar(&clock);
    }

    /// Gets the current compute budget.
    pub fn get_compute_budget(&self) -> Option<ComputeBudget> {
        self.compute_budget
    }

    pub fn get_sigverify(&self) -> bool {
        self.sigverify
    }

    #[cfg(feature = "internal-test")]
    pub fn get_feature_set(&self) -> Arc<FeatureSet> {
        self.feature_set.clone().into()
    }

    fn check_transaction_age(&self, tx: &SanitizedTransaction) -> Result<(), ExecutionResult> {
        self.check_transaction_age_inner(tx)
            .map_err(|e| ExecutionResult {
                tx_result: Err(e),
                ..Default::default()
            })
    }

    fn check_transaction_age_inner(
        &self,
        tx: &SanitizedTransaction,
    ) -> Result<(), TransactionError> {
        let recent_blockhash = tx.message().recent_blockhash();
        if recent_blockhash == &self.latest_blockhash
            || self.check_transaction_for_nonce(
                tx,
                &DurableNonce::from_blockhash(&self.latest_blockhash),
            )
        {
            Ok(())
        } else {
            log::error!(
                "Blockhash {} not found. Expected blockhash {}",
                recent_blockhash,
                self.latest_blockhash
            );
            Err(TransactionError::BlockhashNotFound)
        }
    }

    fn check_transaction_for_nonce(
        &self,
        tx: &SanitizedTransaction,
        next_durable_nonce: &DurableNonce,
    ) -> bool {
        let nonce_is_advanceable = tx.message().recent_blockhash() != next_durable_nonce.as_hash();
        nonce_is_advanceable
    }

    /// Registers a custom syscall in both program runtime environments (v1 and v2).
    ///
    /// **Must be called after `with_builtins()`** (which recreates the environments
    /// from scratch) and **before `with_default_programs()`** (which clones the
    /// environment Arcs into program cache entries, preventing further mutation).
    ///
    /// Panics if the runtime environments cannot be mutated or if registration
    /// fails. This is intentional — a misconfigured syscall should fail loudly
    /// rather than silently.
    pub fn with_custom_syscall(
        mut self,
        name: &str,
        syscall: BuiltinFunction<InvokeContext<'static>>,
    ) -> Self {
        let (Some(program_runtime_v1), Some(program_runtime_v2)) = (
            Arc::get_mut(&mut self.accounts.environments.program_runtime_v1),
            Arc::get_mut(&mut self.accounts.environments.program_runtime_v2),
        ) else {
            panic!("with_custom_syscall: can't mutate program runtimes");
        };

        // Once unregister_function is available, users could replace existing built-in
        // syscalls.

        let _ = program_runtime_v1.unregister_function(name);
        program_runtime_v1
            .register_function(name, syscall)
            .unwrap_or_else(|e| panic!("failed to register syscall '{name}' in runtime_v1: {e}"));

        let _ = program_runtime_v2.unregister_function(name);
        program_runtime_v2
            .register_function(name, syscall)
            .unwrap_or_else(|e| panic!("failed to register syscall '{name}' in runtime_v2: {e}"));

        self
    }
}

struct CheckAndProcessTransactionSuccessCore {
    result: Result<(), TransactionError>,
    compute_units_consumed: u64,
    context: Option<TransactionContext>,
}

struct CheckAndProcessTransactionSuccess {
    core: CheckAndProcessTransactionSuccessCore,
    fee: u64,
    payer_key: Option<Pubkey>,
}

fn execution_result_if_context(
    sanitized_tx: &SanitizedTransaction,
    ctx: TransactionContext,
    result: Result<(), TransactionError>,
    compute_units_consumed: u64,
    fee: u64,
) -> ExecutionResult {
    let (signature, return_data, inner_instructions, post_accounts) =
        execute_tx_helper(sanitized_tx, ctx);
    ExecutionResult {
        tx_result: result,
        signature,
        post_accounts,
        inner_instructions,
        compute_units_consumed,
        return_data,
        included: true,
        fee,
    }
}

fn execute_tx_helper(
    sanitized_tx: &SanitizedTransaction,
    ctx: TransactionContext,
) -> (
    TypedSignature,
    jupnet_sdk::transaction_context::TransactionReturnData,
    InnerInstructionsList,
    Vec<(Pubkey, AccountSharedData)>,
) {
    let signature = sanitized_tx.signature().to_owned();
    let inner_instructions = inner_instructions_list_from_instruction_trace(&ctx);
    let ExecutionRecord {
        accounts,
        return_data,
        touched_account_count: _,
        accounts_resize_delta: _,
    } = ctx.into();
    let msg = sanitized_tx.message();
    let post_accounts = accounts
        .into_iter()
        .enumerate()
        .filter_map(|(idx, pair)| msg.is_writable(idx).then_some(pair))
        .collect();
    (signature, return_data, inner_instructions, post_accounts)
}

fn get_compute_budget_limits(
    sanitized_tx: &SanitizedTransaction,
    _feature_set: &FeatureSet,
) -> Result<ComputeBudgetLimits, ExecutionResult> {
    SVMMessage::get_compute_budget_limits(sanitized_tx).map_err(|e| ExecutionResult {
        tx_result: Err(e),
        ..Default::default()
    })
}

fn calculate_program_indices(
    message: &SVMExecutionMessage,
    transaction_accounts: &mut Vec<TransactionAccount>,
    accounts_db: &AccountsDb,
) -> Result<TransactionProgramIndices, TransactionError> {
    let builtins_start_index = transaction_accounts.len();
    message
        .instructions
        .iter()
        .map(|instruction| {
            let mut account_indices = Vec::with_capacity(1);
            let program_index = instruction.program_id_index;

            let (program_id, is_executable, owner_id) = {
                let (id, account) = transaction_accounts
                    .get(program_index)
                    .expect("program id index must be valid");
                (*id, account.executable(), *account.owner())
            };

            if native_loader::check_id(&program_id) {
                return Ok(account_indices);
            }

            if !is_executable {
                error!("Program account {program_id} is not executable.");
                return Err(TransactionError::InvalidProgramForExecution);
            }

            if native_loader::check_id(&owner_id) {
                account_indices.push(program_index as IndexOfAccount);
                return Ok(account_indices);
            }

            if !transaction_accounts
                .get(builtins_start_index..)
                .ok_or(TransactionError::ProgramAccountNotFound)?
                .iter()
                .any(|(key, _)| *key == owner_id)
            {
                let owner_account = accounts_db.get_account(&owner_id).unwrap();
                if !native_loader::check_id(owner_account.owner()) {
                    error!("Owner account {owner_id} is not owned by the native loader program.");
                    return Err(TransactionError::InvalidProgramForExecution);
                }
                if !owner_account.executable() {
                    error!("Owner account {owner_id} is not executable");
                    return Err(TransactionError::InvalidProgramForExecution);
                }
                transaction_accounts.push((owner_id, owner_account));
            }

            account_indices.push(program_index as IndexOfAccount);
            Ok(account_indices)
        })
        .collect()
}

/// Lighter version of the one in the solana-svm crate.
///
/// Check whether the payer_account is capable of paying the fee. The
/// side effect is to subtract the fee amount from the payer_account
/// balance of lamports. If the payer_acount is not able to pay the
/// fee a specific error is returned.
fn validate_fee_payer(
    payer_address: &Pubkey,
    payer_account: &mut AccountSharedData,
    payer_index: IndexOfAccount,
    rent: &Rent,
    fee: u64,
) -> Result<(), TransactionError> {
    if payer_account.lamports() == 0 {
        error!("Payer account {payer_address} not found.");
        return Err(TransactionError::AccountNotFound);
    }
    let system_account_kind = get_system_account_kind(payer_account).ok_or_else(|| {
        error!("Payer account {payer_address} is not a system account");
        TransactionError::InvalidAccountForFee
    })?;
    let min_balance = match system_account_kind {
        SystemAccountKind::System => 0,
        SystemAccountKind::Nonce => {
            // Should we ever allow a fees charge to zero a nonce account's
            // balance. The state MUST be set to uninitialized in that case
            rent.minimum_balance(jupnet_sdk::nonce::state::State::size())
        }
    };

    let payer_lamports = payer_account.lamports();

    payer_lamports
        .checked_sub(min_balance)
        .and_then(|v| v.checked_sub(fee))
        .ok_or_else(|| {
            error!(
                "Payer account {payer_address} has insufficient lamports for fee. Payer lamports: \
                {payer_lamports} min_balance: {min_balance} fee: {fee}"
            );
            TransactionError::InsufficientFundsForFee
        })?;

    let payer_len = payer_account.data().len();
    let payer_pre_rent_state = get_account_rent_state(rent, payer_account.lamports(), payer_len);
    // we already checked above if we have sufficient balance so this should never error.
    payer_account.checked_sub_lamports(fee).unwrap();

    let payer_post_rent_state = get_account_rent_state(rent, payer_account.lamports(), payer_len);
    check_rent_state_with_account(
        &payer_pre_rent_state,
        &payer_post_rent_state,
        payer_address,
        payer_index,
    )
}

fn map_sanitize_result<F>(
    res: Result<SanitizedTransaction, ExecutionResult>,
    op: F,
) -> ExecutionResult
where
    F: FnOnce(SanitizedTransaction) -> ExecutionResult,
{
    match res {
        Ok(s_tx) => op(s_tx),
        Err(e) => e,
    }
}

#[cfg(feature = "invocation-inspect-callback")]
pub trait InvocationInspectCallback: Send + Sync {
    fn before_invocation(
        &self,
        svm: &LiteSVM,
        tx: &SanitizedTransaction,
        program_indices: &[IndexOfAccount],
        invoke_context: &InvokeContext,
    );

    fn after_invocation(
        &self,
        svm: &LiteSVM,
        invoke_context: &InvokeContext,
        enable_register_tracing: bool,
    );
}

#[cfg(feature = "invocation-inspect-callback")]
pub struct EmptyInvocationInspectCallback;

#[cfg(feature = "invocation-inspect-callback")]
impl InvocationInspectCallback for EmptyInvocationInspectCallback {
    fn before_invocation(
        &self,
        _: &LiteSVM,
        _: &SanitizedTransaction,
        _: &[IndexOfAccount],
        _: &InvokeContext,
    ) {
    }

    fn after_invocation(&self, _: &LiteSVM, _: &InvokeContext, _enable_register_tracing: bool) {}
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        jupnet_sdk::instruction::{AccountMeta, Instruction},
    };

    #[test]
    fn sysvar_accounts_are_demoted_to_readonly() {
        let payer = Keypair::new();
        let svm = LiteSVM::new();
        let rent_key = jupnet_sdk::sysvar::rent::id();
        let ix = Instruction {
            program_id: system_program::id(),
            accounts: vec![AccountMeta {
                pubkey: rent_key,
                is_signer: false,
                is_writable: true,
            }],
            data: vec![],
        };
        let message = Message::new(&[ix], Some(&payer.pubkey()));
        let tx =
            VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&payer]).unwrap();
        let sanitized = svm.sanitize_transaction_no_verify_inner(tx).unwrap();

        assert!(!sanitized.message().is_writable(1));
    }
}
