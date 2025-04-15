/*!
<div align="center">
    <img src="https://raw.githubusercontent.com/litesvm/litesvm/master/logo.jpeg" width="50%" height="50%">
</div>

---

# LiteSVM

[<img alt="github" src="https://img.shields.io/badge/github-LiteSVM/litesvm-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/LiteSVM/litesvm)
[<img alt="crates.io" src="https://img.shields.io/crates/v/litesvm.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/litesvm)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-litesvm-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/litesvm/latest/litesvm/)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/LiteSVM/litesvm/CI.yml?branch=master&style=for-the-badge" height="20">](https://github.com/LiteSVM/litesvm/actions?query=branch%3Amaster)

## 📍 Overview

`litesvm` is a fast and lightweight library for testing Solana programs.
It works by creating an in-process Solana VM optimized for program developers.
This makes it much faster to run and compile than alternatives like `solana-program-test` and `solana-test-validator`.
In a further break from tradition, it has an ergonomic API with sane defaults and extensive configurability for those who want it.

### 🤖 Minimal Example

```rust
use litesvm::LiteSVM;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_system_interface::instruction::transfer;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

let from_keypair = Keypair::new();
let from = from_keypair.pubkey();
let to = Pubkey::new_unique();

let mut svm = LiteSVM::new();
svm.airdrop(&from, 10_000).unwrap();

let instruction = transfer(&from, &to, 64);
let tx = Transaction::new(
    &[&from_keypair],
    Message::new(&[instruction], Some(&from)),
    svm.latest_blockhash(),
);
let tx_res = svm.send_transaction(tx).unwrap();

let from_account = svm.get_account(&from);
let to_account = svm.get_account(&to);
assert_eq!(from_account.unwrap().lamports, 4936);
assert_eq!(to_account.unwrap().lamports, 64);
```

## Deploying Programs

Most of the time we want to do more than just mess around with token transfers -
we want to test our own programs.

**Tip**: if you want to pull a Solana program from mainnet or devnet, use the `solana program dump` command from the Solana CLI.

To add a compiled program to our tests we can use [`.add_program_from_file`](LiteSVM::add_program_from_file).

Here's an example using a [simple program](https://github.com/solana-labs/solana-program-library/tree/bd216c8103cd8eb9f5f32e742973e7afb52f3b81/examples/rust/logging)
from the Solana Program Library that just does some logging:

```rust
use {
    litesvm::LiteSVM,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_pubkey::{pubkey, Pubkey},
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

fn test_logging() {
    let program_id = pubkey!("Logging111111111111111111111111111111111111");
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
    let bytes = include_bytes!("../../node-litesvm/program_bytes/spl_example_logging.so");
    svm.add_program(program_id, bytes);
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    // let's sim it first
    let sim_res = svm.simulate_transaction(tx.clone()).unwrap();
    let meta = svm.send_transaction(tx).unwrap();
    assert_eq!(sim_res.meta, meta);
    assert_eq!(meta.logs[1], "Program log: static string");
    assert!(meta.compute_units_consumed < 10_000) // not being precise here in case it changes
}

```

## Time travel

Many programs rely on the `Clock` sysvar: for example, a mint that doesn't become available until after
a certain time. With `litesvm` you can dynamically overwrite the `Clock` sysvar
using [`svm.set_sysvar::<Clock>()`](LiteSVM::set_sysvar).
Here's an example using a program that panics if `clock.unix_timestamp` is greater than 100
(which is on January 1st 1970):

```rust
use {
    litesvm::LiteSVM,
    solana_clock::Clock,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

fn test_set_clock() {
    let program_id = Pubkey::new_unique();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../node-litesvm/program_bytes/litesvm_clock_example.so");
    svm.add_program(program_id, bytes);
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
        data: vec![1], // unused, this is just to dedup the transaction
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

Here's an example where we give an account a bunch of USDC,
even though we don't have the USDC mint keypair. This is
convenient for testing because it means we don't have to
work with fake USDC in our tests:

```rust
use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::{pubkey, Pubkey},
    spl_associated_token_account_client::address::get_associated_token_address,
    spl_token::{
        state::{Account as TokenAccount, AccountState},
        ID as TOKEN_PROGRAM_ID,
    },
};

fn test_infinite_usdc_mint() {
    let owner = Pubkey::new_unique();
    let usdc_mint = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    let ata = get_associated_token_address(&owner, &usdc_mint);
    let usdc_to_own = 1_000_000_000_000;
    let token_acc = TokenAccount {
        mint: usdc_mint,
        owner: owner,
        amount: usdc_to_own,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut svm = LiteSVM::new();
    let mut token_acc_bytes = [0u8; TokenAccount::LEN];
    TokenAccount::pack(token_acc, &mut token_acc_bytes).unwrap();
    svm.set_account(
        ata,
        Account {
            lamports: 1_000_000_000,
            data: token_acc_bytes.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let raw_account = svm.get_account(&ata).unwrap();
    assert_eq!(
        TokenAccount::unpack(&raw_account.data).unwrap().amount,
        usdc_to_own
    )
}

```

## Copying Accounts from a live environment

If you want to copy accounts from mainnet or devnet, you can use the `solana account` command in the Solana CLI to save account data to a file.

## Other features

Other things you can do with `litesvm` include:

* Changing the max compute units and other compute budget behaviour using [`.with_compute_budget`](LiteSVM::with_compute_budget).
* Disable transaction signature checking using [`.with_sigverify(false)`](LiteSVM::with_sigverify).
* Find previous transactions using [`.get_transaction`](`LiteSVM::get_transaction`).

## When should I use `solana-test-validator`?

While `litesvm` is faster and more convenient, it is also less like a real RPC node.
So `solana-test-validator` is still useful when you need to call RPC methods that LiteSVM
doesn't support, or when you want to test something that depends on real-life validator behaviour
rather than just testing your program and client code.

In general though it is recommended to use `litesvm` wherever possible, as it will make your life
much easier.

*/

#[cfg(feature = "nodejs-internal")]
use qualifier_attr::qualifiers;
#[allow(deprecated)]
use solana_sysvar::recent_blockhashes::IterItem;
#[allow(deprecated)]
use solana_sysvar::{fees::Fees, recent_blockhashes::RecentBlockhashes};
use {
    crate::{
        accounts_db::AccountsDb,
        error::LiteSVMError,
        history::TransactionHistory,
        message_processor::process_message,
        spl::load_spl_programs,
        types::{
            ExecutionResult, FailedTransactionMetadata, TransactionMetadata, TransactionResult,
        },
        utils::{create_blockhash, rent::RentState},
    },
    itertools::Itertools,
    log::error,
    precompiles::load_precompiles,
    solana_account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    solana_bpf_loader_program::syscalls::{
        create_program_runtime_environment_v1, create_program_runtime_environment_v2,
    },
    solana_builtins::BUILTINS,
    solana_clock::Clock,
    solana_compute_budget::{
        compute_budget::ComputeBudget, compute_budget_limits::ComputeBudgetLimits,
    },
    solana_compute_budget_instruction::instructions_processor::process_compute_budget_instructions,
    solana_epoch_rewards::EpochRewards,
    solana_epoch_schedule::EpochSchedule,
    solana_feature_set::FeatureSet,
    solana_fee_structure::FeeStructure,
    solana_hash::Hash,
    solana_keypair::Keypair,
    solana_last_restart_slot::LastRestartSlot,
    solana_log_collector::LogCollector,
    solana_message::{
        inner_instruction::InnerInstructionsList, Message, SanitizedMessage, VersionedMessage,
    },
    solana_native_token::LAMPORTS_PER_SOL,
    solana_nonce::{state::DurableNonce, NONCED_TX_MARKER_IX_INDEX},
    solana_program_runtime::{
        invoke_context::{BuiltinFunctionWithContext, EnvironmentConfig, InvokeContext},
        loaded_programs::{LoadProgramMetrics, ProgramCacheEntry},
    },
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    solana_reserved_account_keys::ReservedAccountKeys,
    solana_sdk_ids::{bpf_loader, native_loader, system_program},
    solana_signature::Signature,
    solana_signer::Signer,
    solana_slot_hashes::SlotHashes,
    solana_slot_history::SlotHistory,
    solana_stake_interface::stake_history::StakeHistory,
    solana_svm_transaction::svm_message::SVMMessage,
    solana_system_program::{get_system_account_kind, SystemAccountKind},
    solana_sysvar::Sysvar,
    solana_sysvar_id::SysvarId,
    solana_timings::ExecuteTimings,
    solana_transaction::{
        sanitized::{MessageHash, SanitizedTransaction},
        versioned::VersionedTransaction,
    },
    solana_transaction_context::{ExecutionRecord, IndexOfAccount, TransactionContext},
    solana_transaction_error::TransactionError,
    std::{cell::RefCell, path::Path, rc::Rc, sync::Arc},
    types::SimulatedTransactionInfo,
    utils::{
        construct_instructions_account,
        inner_instructions::inner_instructions_list_from_instruction_trace,
    },
};

pub mod error;
pub mod types;

mod accounts_db;
mod format_logs;
mod history;
mod message_processor;
mod precompiles;
mod spl;
mod utils;

/// Holds a snapshot of the LiteSVM state.
#[derive(Clone)]
pub struct LiteSVMSnapshot {
    accounts: AccountsDb,
    feature_set: FeatureSet,
    latest_blockhash: Hash,
    history: TransactionHistory,
    compute_budget: Option<ComputeBudget>,
    sigverify: bool,
    blockhash_check: bool,
    fee_structure: FeeStructure,
    log_bytes_limit: Option<usize>,
}

#[derive(Clone)]
pub struct LiteSVM {
    accounts: AccountsDb,
    airdrop_kp: [u8; 64],
    feature_set: FeatureSet,
    latest_blockhash: Hash,
    history: TransactionHistory,
    compute_budget: Option<ComputeBudget>,
    sigverify: bool,
    blockhash_check: bool,
    fee_structure: FeeStructure,
    log_bytes_limit: Option<usize>,
    snapshot: Option<LiteSVMSnapshot>,
}

impl Default for LiteSVM {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            airdrop_kp: Keypair::new().to_bytes(),
            feature_set: Default::default(),
            latest_blockhash: create_blockhash(b"genesis"),
            history: TransactionHistory::new(),
            compute_budget: None,
            sigverify: false,
            blockhash_check: false,
            fee_structure: FeeStructure::default(),
            log_bytes_limit: Some(10_000),
            snapshot: None,
        }
    }
}

impl LiteSVM {
    /// Creates the basic test environment.
    pub fn new() -> Self {
        LiteSVM::default()
            .with_feature_set(FeatureSet::all_enabled())
            .with_builtins()
            .with_lamports(1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL))
            .with_sysvars()
            .with_precompiles()
            .with_spl_programs()
            .with_sigverify(true)
            .with_blockhash_check(true)
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
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_builtins(&mut self) {
        BUILTINS.iter().for_each(|builtint| {
            if builtint
                .enable_feature_id
                .map_or(true, |x| self.feature_set.is_active(&x))
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

        let program_runtime_v1 = create_program_runtime_environment_v1(
            &self.feature_set,
            &ComputeBudget::default(),
            false,
            false,
        )
        .unwrap();

        let program_runtime_v2 =
            create_program_runtime_environment_v2(&ComputeBudget::default(), true);

        self.accounts.programs_cache.environments.program_runtime_v1 = Arc::new(program_runtime_v1);
        self.accounts.programs_cache.environments.program_runtime_v2 = Arc::new(program_runtime_v2);
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
            Keypair::from_bytes(&self.airdrop_kp).unwrap().pubkey(),
            AccountSharedData::new(lamports, 0, &system_program::id()),
        );
    }

    /// Changes the initial lamports in LiteSVM's airdrop account.
    pub fn with_lamports(mut self, lamports: u64) -> Self {
        self.set_lamports(lamports);
        self
    }

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_spl_programs(&mut self) {
        load_spl_programs(self);
    }

    /// Includes the standard SPL programs.
    pub fn with_spl_programs(mut self) -> Self {
        self.set_spl_programs();
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

    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    fn set_precompiles(&mut self) {
        load_precompiles(self);
    }

    /// Adds the standard precompiles to the VM.
    //
    // Use `with_feature_set` beforehand to change change what precompiles are added.
    pub fn with_precompiles(mut self) -> Self {
        self.set_precompiles();
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
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        self.accounts.get_account(pubkey).map(Into::into)
    }

    /// Sets all information associated with the account of the provided pubkey.
    pub fn set_account(&mut self, pubkey: Pubkey, data: Account) -> Result<(), LiteSVMError> {
        self.accounts.add_account(pubkey, data.into())
    }

    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&self, pubkey: &Pubkey) -> Option<u64> {
        self.accounts.get_account(pubkey).map(|x| x.lamports())
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
        let account =
            AccountSharedData::new_data(1, &sysvar, &solana_sdk_ids::sysvar::id()).unwrap();
        self.accounts.add_account(T::id(), account).unwrap();
    }

    /// Gets a sysvar from the test environment.
    pub fn get_sysvar<T>(&self) -> T
    where
        T: Sysvar + SysvarId,
    {
        bincode::deserialize(self.accounts.get_account(&T::id()).unwrap().data()).unwrap()
    }

    /// Gets a transaction from the transaction history.
    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionResult> {
        self.history.get_transaction(signature)
    }

    /// Airdrops the account with the lamports specified.
    pub fn airdrop(&mut self, pubkey: &Pubkey, lamports: u64) -> TransactionResult {
        let payer = Keypair::from_bytes(&self.airdrop_kp).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(Message::new_with_blockhash(
                &[solana_system_interface::instruction::transfer(
                    &payer.pubkey(),
                    pubkey,
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

        let mut account = AccountSharedData::new(0, 1, &bpf_loader::id());
        account.set_executable(true);
        self.accounts.add_account(program_id, account).unwrap();
    }

    /// Adds an SBF program to the test environment from the file specified.
    pub fn add_program_from_file(
        &mut self,
        program_id: Pubkey,
        path: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        let bytes = std::fs::read(path)?;
        self.add_program(program_id, &bytes);
        Ok(())
    }

    /// Adds am SBF program to the test environment.
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
            None,
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

    fn create_transaction_context(
        &self,
        compute_budget: ComputeBudget,
        accounts: Vec<(Pubkey, AccountSharedData)>,
    ) -> TransactionContext {
        TransactionContext::new(
            accounts,
            self.get_sysvar(),
            compute_budget.max_instruction_stack_depth,
            compute_budget.max_instruction_trace_length,
        )
    }

    fn sanitize_transaction_no_verify_inner(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        let res = SanitizedTransaction::try_create(
            tx,
            MessageHash::Compute,
            Some(false),
            &self.accounts,
            &ReservedAccountKeys::empty_key_set(),
        );
        res.map_err(|e| {
            log::error!("Transaction sanitization failed");
            e
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
        tx.verify_precompiles(&self.feature_set)?;

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
        let compute_budget = self.compute_budget.unwrap_or_else(|| ComputeBudget {
            compute_unit_limit: u64::from(compute_budget_limits.compute_unit_limit),
            heap_size: compute_budget_limits.updated_heap_bytes,
            ..ComputeBudget::default()
        });
        let blockhash = tx.message().recent_blockhash();
        //reload program cache
        let mut program_cache_for_tx_batch = self.accounts.programs_cache.clone();
        let mut accumulated_consume_units = 0;
        let message = tx.message();
        let account_keys = message.account_keys();
        let instruction_accounts = message
            .instructions()
            .iter()
            .flat_map(|instruction| &instruction.accounts)
            .unique()
            .collect::<Vec<&u8>>();
        let fee = solana_fee::calculate_fee(
            message,
            false,
            self.fee_structure.lamports_per_signature,
            0,
            solana_fee::FeeFeatures::from(&self.feature_set),
        );
        let mut validated_fee_payer = false;
        let mut payer_key = None;
        let maybe_accounts = account_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let mut account_found = true;
                let account = if solana_sdk_ids::sysvar::instructions::check_id(key) {
                    construct_instructions_account(message)
                } else {
                    let instruction_account = u8::try_from(i)
                        .map(|i| instruction_accounts.contains(&&i))
                        .unwrap_or(false);
                    let mut account = if !instruction_account
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
                    };
                    if !validated_fee_payer
                        && (!message.is_invoked(i) || message.is_instruction_account(i))
                    {
                        validate_fee_payer(
                            key,
                            &mut account,
                            i as IndexOfAccount,
                            &self.accounts.sysvar_cache.get_rent().unwrap(),
                            fee,
                        )?;
                        validated_fee_payer = true;
                        payer_key = Some(*key);
                    }
                    account
                };

                Ok((*key, account))
            })
            .collect::<solana_transaction_error::TransactionResult<Vec<_>>>();
        let mut accounts = match maybe_accounts {
            Ok(accs) => accs,
            Err(e) => {
                return (Err(e), accumulated_consume_units, None, fee, payer_key);
            }
        };
        if !validated_fee_payer {
            error!("Failed to validate fee payer");
            return (
                Err(TransactionError::AccountNotFound),
                accumulated_consume_units,
                None,
                fee,
                payer_key,
            );
        }
        let builtins_start_index = accounts.len();
        let maybe_program_indices = tx
            .message()
            .instructions()
            .iter()
            .map(|c| {
                let mut account_indices: Vec<u16> = Vec::with_capacity(2);
                let program_index = c.program_id_index as usize;
                // This may never error, because the transaction is sanitized
                let (program_id, program_account) = accounts.get(program_index).unwrap();
                if native_loader::check_id(program_id) {
                    return Ok(account_indices);
                }
                if !program_account.executable() {
                    error!("Program account {program_id} is not executable.");
                    return Err(TransactionError::InvalidProgramForExecution);
                }
                account_indices.insert(0, program_index as IndexOfAccount);

                let owner_id = program_account.owner();
                if native_loader::check_id(owner_id) {
                    return Ok(account_indices);
                }
                if !accounts
                    .get(builtins_start_index..)
                    .ok_or(TransactionError::ProgramAccountNotFound)?
                    .iter()
                    .any(|(key, _)| key == owner_id)
                {
                    let owner_account = self.get_account(owner_id).unwrap();
                    if !native_loader::check_id(owner_account.owner()) {
                        error!(
                            "Owner account {owner_id} is not owned by the native loader program."
                        );
                        return Err(TransactionError::InvalidProgramForExecution);
                    }
                    if !owner_account.executable {
                        error!("Owner account {owner_id} is not executable");
                        return Err(TransactionError::InvalidProgramForExecution);
                    }
                    accounts.push((*owner_id, owner_account.into()));
                }
                Ok(account_indices)
            })
            .collect::<Result<Vec<Vec<u16>>, TransactionError>>();
        match maybe_program_indices {
            Ok(program_indices) => {
                let mut context = self.create_transaction_context(compute_budget, accounts);
                let mut tx_result = process_message(
                    tx.message(),
                    &program_indices,
                    &mut InvokeContext::new(
                        &mut context,
                        &mut program_cache_for_tx_batch,
                        EnvironmentConfig::new(
                            *blockhash,
                            self.fee_structure.lamports_per_signature,
                            0,
                            &|_| 0,
                            Arc::new(self.feature_set.clone()),
                            &self.accounts.sysvar_cache,
                        ),
                        Some(log_collector),
                        compute_budget,
                    ),
                    &mut ExecuteTimings::default(),
                    &mut accumulated_consume_units,
                )
                .map(|_| ());

                if let Err(err) = self.check_accounts_rent(tx, &context) {
                    tx_result = Err(err);
                };

                (
                    tx_result,
                    accumulated_consume_units,
                    Some(context),
                    fee,
                    payer_key,
                )
            }
            Err(e) => (Err(e), accumulated_consume_units, None, fee, payer_key),
        }
    }

    fn check_accounts_rent(
        &self,
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

    fn execute_transaction_no_verify(
        &mut self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction_no_verify(tx), |s_tx| {
            self.execute_sanitized_transaction(s_tx, log_collector)
        })
    }

    fn execute_transaction(
        &mut self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction(tx), |s_tx| {
            self.execute_sanitized_transaction(s_tx, log_collector)
        })
    }

    fn execute_sanitized_transaction(
        &mut self,
        sanitized_tx: SanitizedTransaction,
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
        } = match self.check_and_process_transaction(&sanitized_tx, log_collector) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            let tx_result = self.check_tx_result(result, payer_key, fee);
            execution_result_if_context(sanitized_tx, ctx, tx_result, compute_units_consumed)
        } else {
            ExecutionResult::result_and_compute_units(result, compute_units_consumed)
        }
    }

    fn execute_sanitized_transaction_readonly(
        &self,
        sanitized_tx: SanitizedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        let CheckAndProcessTransactionSuccess {
            core:
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                },
            ..
        } = match self.check_and_process_transaction(&sanitized_tx, log_collector) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            execution_result_if_context(sanitized_tx, ctx, result, compute_units_consumed)
        } else {
            ExecutionResult::result_and_compute_units(result, compute_units_consumed)
        }
    }

    fn check_tx_result(
        &mut self,
        result: Result<(), TransactionError>,
        payer_key: Option<Pubkey>,
        fee: u64,
    ) -> Result<(), TransactionError> {
        if result.is_ok() {
            result
        } else if let Some(payer) = payer_key {
            self.accounts.withdraw(&payer, fee).and(result)
        } else {
            result
        }
    }

    fn check_and_process_transaction(
        &self,
        sanitized_tx: &SanitizedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> Result<CheckAndProcessTransactionSuccess, ExecutionResult> {
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
        if self.history.check_transaction(sanitized_tx.signature()) {
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
            self.execute_sanitized_transaction_readonly(s_tx, log_collector)
        })
    }

    fn execute_transaction_no_verify_readonly(
        &self,
        tx: VersionedTransaction,
        log_collector: Rc<RefCell<LogCollector>>,
    ) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction_no_verify(tx), |s_tx| {
            self.execute_sanitized_transaction_readonly(s_tx, log_collector)
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
            signature,
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
        Arc::new(self.feature_set.clone())
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
    ) -> solana_transaction_error::TransactionResult<()> {
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

    fn check_message_for_nonce(&self, message: &SanitizedMessage) -> bool {
        message
            .get_durable_nonce()
            .and_then(|nonce_address| self.accounts.get_account(nonce_address))
            .and_then(|nonce_account| {
                solana_nonce_account::verify_nonce_account(
                    &nonce_account,
                    message.recent_blockhash(),
                )
            })
            .is_some_and(|nonce_data| {
                message
                    .get_ix_signers(NONCED_TX_MARKER_IX_INDEX as usize)
                    .any(|signer| signer == &nonce_data.authority)
            })
    }

    fn check_transaction_for_nonce(
        &self,
        tx: &SanitizedTransaction,
        next_durable_nonce: &DurableNonce,
    ) -> bool {
        let nonce_is_advanceable = tx.message().recent_blockhash() != next_durable_nonce.as_hash();
        nonce_is_advanceable && self.check_message_for_nonce(tx.message())
    }

    /// Creates a snapshot of the current SVM state.
    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    pub fn snapshot(&mut self) {
        self.snapshot = Some(LiteSVMSnapshot {
            accounts: self.accounts.clone(),
            feature_set: self.feature_set.clone(),
            latest_blockhash: self.latest_blockhash, // Hash is Copy
            history: self.history.clone(),
            compute_budget: self.compute_budget,
            sigverify: self.sigverify,
            blockhash_check: self.blockhash_check,
            fee_structure: self.fee_structure.clone(),
            log_bytes_limit: self.log_bytes_limit,
        })
    }

    /// Reverts the SVM state to a previously created snapshot.
    #[cfg_attr(feature = "nodejs-internal", qualifiers(pub))]
    pub fn revert(&mut self) {
        let Some(snapshot) = self.snapshot.clone() else {
          return;
        };

        self.accounts = snapshot.accounts;
        self.feature_set = snapshot.feature_set;
        self.latest_blockhash = snapshot.latest_blockhash;
        self.history = snapshot.history;
        self.compute_budget = snapshot.compute_budget;
        self.sigverify = snapshot.sigverify;
        self.blockhash_check = snapshot.blockhash_check;
        self.fee_structure = snapshot.fee_structure;
        self.log_bytes_limit = snapshot.log_bytes_limit;
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
    sanitized_tx: SanitizedTransaction,
    ctx: TransactionContext,
    result: Result<(), TransactionError>,
    compute_units_consumed: u64,
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
    }
}

fn execute_tx_helper(
    sanitized_tx: SanitizedTransaction,
    ctx: TransactionContext,
) -> (
    Signature,
    solana_transaction_context::TransactionReturnData,
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
    feature_set: &FeatureSet,
) -> Result<ComputeBudgetLimits, ExecutionResult> {
    process_compute_budget_instructions(
        SVMMessage::program_instructions_iter(sanitized_tx),
        feature_set,
    )
    .map_err(|e| ExecutionResult {
        tx_result: Err(e),
        ..Default::default()
    })
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
) -> solana_transaction_error::TransactionResult<()> {
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
            rent.minimum_balance(solana_nonce::state::State::size())
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

    let payer_pre_rent_state = RentState::from_account(payer_account, rent);
    // we already checked above if we have sufficient balance so this should never error.
    payer_account.checked_sub_lamports(fee).unwrap();

    let payer_post_rent_state = RentState::from_account(payer_account, rent);
    check_rent_state_with_account(
        &payer_pre_rent_state,
        &payer_post_rent_state,
        payer_address,
        payer_index,
    )
}

// modified version of the private fn in solana-svm
fn check_rent_state_with_account(
    pre_rent_state: &RentState,
    post_rent_state: &RentState,
    address: &Pubkey,
    account_index: IndexOfAccount,
) -> solana_transaction_error::TransactionResult<()> {
    if !solana_sdk_ids::incinerator::check_id(address)
        && !post_rent_state.transition_allowed_from(pre_rent_state)
    {
        let account_index = account_index as u8;
        error!("Transaction would leave account {address} with insufficient funds for rent");
        Err(TransactionError::InsufficientFundsForRent { account_index })
    } else {
        Ok(())
    }
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
