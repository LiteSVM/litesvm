use {
    crate::fixture::{
        accounts,
        backend::Backend,
        outcome::{mint_supply, token_amount, TrackedAccount},
        Account, CtxBuilder, Fixture, Instruction, Outcome, Pubkey, SetupError, TokenProgram,
    },
    std::{ops::Deref, path::Path},
    wincode::{config::DefaultConfig, SchemaRead, SchemaWrite},
};

/// Anything executable as one transaction: a single instruction builder, or a
/// chain as a tuple, array, or `Vec` — `test.execute((deposit, withdraw))`.
/// The `M` marker only steers inference; call sites never name it.
pub trait IntoInstructions<M> {
    /// The transaction's instructions, in order.
    fn into_instructions(self) -> Vec<Instruction>;
}

/// Marker for a single instruction.
pub struct One;

/// Marker for an instruction chain.
pub struct Many;

impl<T: Into<Instruction>> IntoInstructions<One> for T {
    fn into_instructions(self) -> Vec<Instruction> {
        vec![self.into()]
    }
}

impl<T: Into<Instruction>, const N: usize> IntoInstructions<Many> for [T; N] {
    fn into_instructions(self) -> Vec<Instruction> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T: Into<Instruction>> IntoInstructions<Many> for Vec<T> {
    fn into_instructions(self) -> Vec<Instruction> {
        self.into_iter().map(Into::into).collect()
    }
}

macro_rules! impl_into_instructions_for_tuple {
    ($($name:ident),+) => {
        impl<$($name: Into<Instruction>),+> IntoInstructions<Many> for ($($name,)+) {
            fn into_instructions(self) -> Vec<Instruction> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                vec![$($name.into()),+]
            }
        }
    };
}

impl_into_instructions_for_tuple!(A, B);
impl_into_instructions_for_tuple!(A, B, C);
impl_into_instructions_for_tuple!(A, B, C, D);
impl_into_instructions_for_tuple!(A, B, C, D, E);

/// Default balance assigned by [`crate::fixture::Wallet`]: ten SOL.
pub const DEFAULT_WALLET_LAMPORTS: u64 = 10_000_000_000;

/// An isolated Solana program test world.
///
/// Each `Ctx` owns its runtime and account state. The public API describes
/// test behavior rather than a particular SVM, so the same test can be hosted
/// by additional runtimes without becoming generic over a backend.
pub struct Ctx {
    pub(super) backend: Backend,
    pub(super) program_id: Pubkey,
    pub(super) program_path: std::path::PathBuf,
    pub(super) fresh_addresses: u64,
    /// RPC endpoint used to fill the `Dump` store on a miss.
    pub(super) rpc_url: String,
    /// Project directory whose `.parallax/` store this world uses, when set
    /// explicitly (by the `#[parallax_test]` macro or the builder).
    pub(super) project_dir: Option<String>,
    /// Addresses installed by a `Dump`, tracked for guided errors.
    pub(super) dumped_addresses: Vec<Pubkey>,
    /// Observed slots of installed dump entries, tracked for mixed-slot
    /// coherence warnings.
    pub(super) dumped_slots: Vec<u64>,
    /// Whether the mixed-slot coherence warning has already fired for this world.
    pub(super) dump_warned: bool,
    /// Checks verified against every committed execution's outcome.
    pub(super) invariants: Vec<crate::fixture::CheckFn>,
}

impl Ctx {
    /// Assemble a world from its loaded backend and its dump configuration.
    pub(crate) fn from_parts(
        backend: Backend,
        program_id: Pubkey,
        program_path: std::path::PathBuf,
        rpc_url: String,
        project_dir: Option<String>,
    ) -> Self {
        Self {
            backend,
            program_id,
            program_path,
            fresh_addresses: 0,
            rpc_url,
            project_dir,
            dumped_addresses: Vec::new(),
            dumped_slots: Vec::new(),
            dump_warned: false,
            invariants: Vec::new(),
        }
    }

    /// Load a compiled program by discovering its artifact.
    ///
    /// # Panics
    ///
    /// Panics with an actionable message when no program artifact can be
    /// located or read. Use [`Self::try_new`] to handle setup errors.
    pub fn new(program_id: impl Into<Pubkey>) -> Self {
        Self::builder(program_id)
            .build()
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Fallible variant of [`Self::new`].
    pub fn try_new(program_id: impl Into<Pubkey>) -> Result<Self, SetupError> {
        Self::builder(program_id).build()
    }

    /// Configure artifact discovery and runtime limits before loading a world.
    pub fn builder(program_id: impl Into<Pubkey>) -> CtxBuilder {
        CtxBuilder::new(program_id.into())
    }

    /// The primary program under test.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// The ELF artifact loaded for the primary program.
    pub fn program_path(&self) -> &Path {
        &self.program_path
    }

    /// Install a built-in or application-defined fixture.
    pub fn add<F: Fixture>(&mut self, fixture: F) -> F::Output {
        fixture.install(self)
    }

    /// Insert or replace a raw account.
    pub fn set_account(&mut self, account: Account) {
        self.backend.set_account(account);
    }

    /// Read a raw account from the current world.
    pub fn account(&self, address: Pubkey) -> Option<Account> {
        self.backend.account(&address)
    }

    /// Decode an account with a caller-supplied decoder (e.g. a generated
    /// client's decode function).
    pub fn account_as<T>(
        &self,
        address: Pubkey,
        decode: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Option<T> {
        self.account(address)
            .and_then(|account| decode(&account.data))
    }

    /// Preload a program's compiled **bytes** for cross-program invocations.
    ///
    /// Takes the program id and its in-memory ELF. Contrast with
    /// [`Dump::program`](crate::fixture::Dump::program), which fetches a program
    /// over the network, and
    /// [`Load::program`](crate::fixture::Load::program), which reads one from a
    /// dump file on disk.
    pub fn preload_program(&mut self, program_id: Pubkey, elf: &[u8]) {
        self.backend.load_program(&program_id, elf);
    }

    /// Produce a deterministic address unused by earlier fixtures in this
    /// world. The sequence is independent for every test, and identical across
    /// the Rust and TypeScript harnesses so fixture addresses match. Internal:
    /// fixtures call this to place accounts the caller did not pin; tests name
    /// actors through [`crate::fixture::Wallet`] and read back the address.
    pub(crate) fn fresh_address(&mut self) -> Pubkey {
        self.fresh_addresses += 1;
        let mut bytes = *b"parallax/fresh-address\0\0\0\0\0\0\0\0\0\0";
        bytes[24..].copy_from_slice(&self.fresh_addresses.to_le_bytes());
        Pubkey::new_from_array(bytes)
    }

    /// Derive an associated-token address without installing the account.
    pub fn derive_ata(&self, owner: Pubkey, mint: Pubkey, token_program: TokenProgram) -> Pubkey {
        Pubkey::find_program_address(
            &[owner.as_ref(), token_program.id().as_ref(), mint.as_ref()],
            &crate::fixture::SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .0
    }

    /// Derive a program-derived address under the program under test from raw
    /// seed slices.
    pub fn derive_pda(&self, seeds: &[&[u8]]) -> Pubkey {
        self.derive_pda_with_bump(seeds).0
    }

    /// Derive a program-derived address and its canonical bump under the
    /// program under test from raw seed slices.
    pub fn derive_pda_with_bump(&self, seeds: &[&[u8]]) -> (Pubkey, u8) {
        Pubkey::find_program_address(seeds, &self.program_id)
    }

    /// Decode the account at `address` with wincode's [`DefaultConfig`].
    ///
    /// `T`'s wincode schema is applied to the account's full data. A generated
    /// client account type encodes its discriminator as the leading schema
    /// field, so an on-chain account round-trips directly — the same bytes a
    /// program wrote, decoded back into the client type. For a type whose
    /// schema covers only a suffix of the data, use [`Self::read_at`].
    ///
    /// The schema frames bytes only and carries no ownership, so a read does not
    /// check the account's owner. This differs from the TypeScript harness by
    /// design: there, codecs carry and validate `owner` because generated
    /// bundles are self-framing; in Rust owner stays an orthogonal
    /// [`Account::owner`](crate::fixture::Account::owner) check.
    ///
    /// # Trailing bytes
    ///
    /// `T` must consume the account. wincode reads exactly `T`'s bytes and
    /// stops; any tail left over must be **all zero** — Solana's
    /// zero-initialized reserved padding, as a growable or migration-target
    /// account carries for future fields. A *non-zero* unconsumed byte is the
    /// fingerprint of the wrong or a stale type read against the account and
    /// **panics**, rather than silently returning a value decoded from a prefix.
    /// (Every macro-generated account is sized to exactly its schema, so this is
    /// invisible in the common case; it only fires on a genuine mismatch.) To
    /// read a fixed prefix and ignore a non-zero tail deliberately, fetch the
    /// raw bytes with [`Self::account`] and decode them yourself.
    ///
    /// # Panics
    ///
    /// Panics with the address and the wincode error when no account exists at
    /// `address` or its bytes do not decode as `T`, and when a non-zero tail
    /// remains after `T` (see *Trailing bytes*).
    pub fn read<T>(&self, address: Pubkey) -> Snapshot<T>
    where
        T: for<'de> SchemaRead<'de, DefaultConfig, Dst = T>,
    {
        self.read_at(address, 0)
    }

    /// Decode the account at `address` starting `offset` bytes in.
    ///
    /// Use when `T`'s schema describes only a suffix of the account — for
    /// example decoding the fields after a discriminator the caller frames
    /// separately.
    ///
    /// The same trailing-bytes contract as [`Self::read`] applies to the region
    /// *after* `offset`: `T` must consume it, save for a zeroed reserved-padding
    /// tail; a non-zero unconsumed byte past `T` panics. `offset` frames the
    /// bytes before `T` (a discriminator); it does not license a non-zero suffix
    /// after `T`.
    ///
    /// # Panics
    ///
    /// Panics like [`Self::read`] — including on a non-zero tail after `T` — and
    /// additionally when the account holds fewer than `offset` bytes.
    pub fn read_at<T>(&self, address: Pubkey, offset: usize) -> Snapshot<T>
    where
        T: for<'de> SchemaRead<'de, DefaultConfig, Dst = T>,
    {
        let account = self.account(address).unwrap_or_else(|| {
            panic!(
                "read {}: no account at {address}",
                core::any::type_name::<T>()
            )
        });
        let state = decode::<T>("read", address, &account.data, offset);
        Snapshot {
            address,
            lamports: account.lamports,
            state,
        }
    }

    /// Serialize `value` with wincode and install it as a rent-exempt account
    /// owned by `owner`.
    ///
    /// The value's schema frames the account exactly as the program writes it:
    /// a generated account type emits its discriminator as the leading schema
    /// field, so no separate framing is needed. `owner` is explicit because the
    /// serialization substrate carries no ownership of its own. Returns
    /// `address`.
    ///
    /// Note the asymmetry with [`Self::read`], which takes only an address:
    /// `owner` is required to *install* the account (every Solana account has
    /// one) but is never validated by a read. Pair a read with
    /// [`Account::owner`](crate::fixture::Account::owner) when ownership matters.
    pub fn write<T>(&mut self, address: Pubkey, owner: Pubkey, value: T) -> Pubkey
    where
        T: SchemaWrite<DefaultConfig, Src = T>,
    {
        let data = wincode::serialize(&value)
            .unwrap_or_else(|error| panic!("write {}: {error:?}", core::any::type_name::<T>()));
        self.set_account(accounts::program_account(address, owner, data));
        address
    }

    /// An account's current lamport balance.
    pub fn lamports(&self, address: Pubkey) -> u64 {
        self.required_account(address).lamports
    }

    /// A base Token or Token-2022 account's current amount.
    pub fn tokens(&self, address: Pubkey) -> u64 {
        token_amount(&self.required_account(address))
    }

    /// A base Token or Token-2022 mint's current supply.
    pub fn supply(&self, address: Pubkey) -> u64 {
        mint_supply(&self.required_account(address))
    }

    /// Set the runtime clock's Unix timestamp.
    pub fn warp_to_timestamp(&mut self, timestamp: i64) {
        self.backend.warp_to_timestamp(timestamp);
    }

    /// Set the transaction compute-unit limit for this world.
    ///
    /// The builder equivalent is [`CtxBuilder::compute_unit_limit`]; this
    /// reconfigures the budget on an already-built world, preserving every
    /// loaded program and installed account.
    ///
    /// [`CtxBuilder::compute_unit_limit`]: crate::fixture::CtxBuilder::compute_unit_limit
    pub fn set_compute_unit_limit(&mut self, limit: u64) {
        self.backend.set_compute_unit_limit(limit);
    }

    /// Register a [`CheckFn`](crate::fixture::CheckFn) verified after every
    /// successful committed execution.
    ///
    /// Define a protocol invariant once — a fact, a [`bundle`](crate::fixture::bundle),
    /// or a [`CheckFn::new`](crate::fixture::CheckFn::new) closure — and every
    /// succeeding `execute` in the test enforces it against the same witness the
    /// test would check. Failed sends commit nothing (an invariant that held
    /// before still holds) and simulations never run invariants. An invariant
    /// sees the transaction only: the accounts it reads must be part of it.
    pub fn invariant(&mut self, check: crate::fixture::CheckFn) {
        self.invariants.push(check);
    }

    /// Execute and commit one transaction: a single instruction, or a chain
    /// as a tuple, array, or `Vec`.
    pub fn execute<M>(&mut self, instructions: impl IntoInstructions<M>) -> Outcome {
        self.run(instructions.into_instructions(), Vec::new(), true)
    }

    /// Execute and commit with raw transaction-input accounts seeding or
    /// overriding world state — useful when malformed input *is* the test.
    pub fn execute_with<M>(
        &mut self,
        instructions: impl IntoInstructions<M>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Outcome {
        self.run(
            instructions.into_instructions(),
            accounts.into_iter().collect(),
            true,
        )
    }

    /// Execute one transaction without committing its changes.
    pub fn simulate<M>(&mut self, instructions: impl IntoInstructions<M>) -> Outcome {
        self.run(instructions.into_instructions(), Vec::new(), false)
    }

    /// Simulate with raw transaction-input accounts, committing nothing.
    pub fn simulate_with<M>(
        &mut self,
        instructions: impl IntoInstructions<M>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Outcome {
        self.run(
            instructions.into_instructions(),
            accounts.into_iter().collect(),
            false,
        )
    }

    fn run(
        &mut self,
        instructions: impl AsRef<[Instruction]>,
        mut inputs: Vec<Account>,
        commit: bool,
    ) -> Outcome {
        let instructions = instructions.as_ref();
        assert!(
            !instructions.is_empty(),
            "a transaction needs an instruction"
        );
        assert_unique_accounts(&inputs);

        let mut tracked = Vec::<TrackedAccount>::new();
        for instruction in instructions {
            for meta in &instruction.accounts {
                if let Some(existing) = tracked
                    .iter_mut()
                    .find(|account| account.address == meta.pubkey)
                {
                    existing.writable |= meta.is_writable;
                    existing.signer |= meta.is_signer;
                    continue;
                }
                let before = inputs
                    .iter()
                    .find(|account| account.address == meta.pubkey)
                    .cloned()
                    .or_else(|| self.backend.account(&meta.pubkey));
                tracked.push(TrackedAccount {
                    address: meta.pubkey,
                    writable: meta.is_writable,
                    signer: meta.is_signer,
                    before,
                    after: None,
                });
            }
        }

        for input in &inputs {
            if tracked
                .iter()
                .all(|account| account.address != input.address)
            {
                tracked.push(TrackedAccount {
                    address: input.address,
                    writable: false,
                    signer: false,
                    before: Some(input.clone()),
                    after: None,
                });
            }
        }

        // Backfill accounts a transaction names but the world has not
        // installed. A missing writable account is an init target — including
        // keypair accounts that sign their own creation — and enters as
        // Solana's empty system account, exactly as a brand-new keypair
        // account arrives on chain; the SVM commits that input only when
        // execution succeeds, so init targets persist without polluting the
        // world after a failed transaction. A missing read-only signer (a
        // co-signer, e.g. a multisig member) enters as a funded system
        // account, matching the real wallets those signatures come from.
        // Actors that pay — payers, makers — are world state: install them
        // with [`crate::fixture::Wallet`].
        for account in &tracked {
            // A present pre-state means the account is installed or was supplied
            // as an explicit input, so it needs no backfill. Every remaining
            // tracked address is unique and absent from `inputs`.
            if account.before.is_some() {
                continue;
            }
            if account.writable {
                inputs.push(accounts::empty_account(account.address));
            } else if account.signer {
                inputs.push(accounts::system_account(
                    account.address,
                    DEFAULT_WALLET_LAMPORTS,
                ));
            }
        }

        let result = self.backend.execute(instructions, &inputs, commit);
        let succeeded = result.is_ok();
        for account in &mut tracked {
            account.after = if !succeeded {
                account.before.clone()
            } else if commit {
                // An installed read-only account cannot change, so its committed
                // post-state is its pre-state and needs no read back. Writable
                // accounts may have changed, and a backfilled read-only signer (a
                // co-signer with no pre-state) was seeded funded, so both are read
                // from the store.
                if account.writable || (account.signer && account.before.is_none()) {
                    self.backend.account(&account.address)
                } else {
                    account.before.clone()
                }
            } else {
                // Simulation reports only writable accounts; a read-only
                // account cannot change, so its pre-state is its post-state.
                Outcome::simulated_account(&result, &account.address)
                    .or_else(|| account.before.clone())
            };
        }

        // Guided error: in a world that dumped mainnet accounts, a failure on an
        // account the world never installed is very often a missing dump. Name
        // the first such read-only account (writable → init target, signer →
        // funded co-signer, so neither is "missing"). Non-noisy: only when the
        // world has dumps and the transaction actually failed.
        let hint = (!succeeded && self.has_dumps())
            .then(|| {
                tracked
                    .iter()
                    .find(|account| {
                        account.before.is_none() && !account.writable && !account.signer
                    })
                    .map(|account| account.address)
            })
            .flatten();
        let outcome = Outcome::from_backend(result, tracked)
            .with_hint(crate::fixture::dump::missing_account_hint(hint));
        if commit && outcome.is_ok() {
            for invariant in &self.invariants {
                invariant.run(&outcome);
            }
        }
        outcome
    }

    fn required_account(&self, address: Pubkey) -> Account {
        self.account(address)
            .unwrap_or_else(|| panic!("no account at {address}"))
    }
}

fn assert_unique_accounts(accounts: &[Account]) {
    for (index, account) in accounts.iter().enumerate() {
        assert!(
            accounts[..index]
                .iter()
                .all(|earlier| earlier.address != account.address),
            "transaction input contains account {} more than once",
            account.address
        );
    }
}

/// Decode `data` (from `offset`) as `T` via wincode's [`DefaultConfig`]. Shared
/// by [`Ctx::read`] and [`Ctx::read_at`] so
/// every typed read takes the same decode path. `context` names the calling
/// operation so panics stay actionable.
///
/// wincode reads exactly `T`'s serialized bytes and stops, so a schema shorter
/// than the account leaves a tail. This enforces the [`Ctx::read`] contract:
/// the tail must be all zero — Solana's zero-initialized reserved padding, the
/// one legitimate longer-than-typed shape (a growable or migration-target
/// account). A *non-zero* unconsumed byte is the fingerprint of the wrong or a
/// stale type read against the account, and panics rather than silently
/// returning a short read.
pub(crate) fn decode<T>(context: &str, address: Pubkey, data: &[u8], offset: usize) -> T
where
    T: for<'de> SchemaRead<'de, DefaultConfig, Dst = T>,
{
    let name = core::any::type_name::<T>();
    let mut cursor = data.get(offset..).unwrap_or_else(|| {
        panic!(
            "{context} {name}: account {address} holds {} bytes, need at least {offset}",
            data.len()
        )
    });
    // Read through `&mut &[u8]` so the cursor advances to the unconsumed tail,
    // exactly as wincode's own `deserialize_exact` does — then inspect that tail
    // instead of merely requiring it to be empty.
    let value = <T as SchemaRead<'_, DefaultConfig>>::get(&mut cursor).unwrap_or_else(|error| {
        panic!("{context} {name}: account {address} did not decode as {name}: {error:?}")
    });
    if let Some(first_nonzero) = cursor.iter().position(|&byte| byte != 0) {
        panic!(
            "{context} {name}: account {address} has {trailing} trailing byte(s) past the \
             decoded {name} (first non-zero at tail offset {first_nonzero}); read/read_at allow \
             only zeroed reserved padding after the value — read the correct type, or frame a \
             suffix with read_at",
            trailing = cursor.len(),
        );
    }
    value
}

/// Typed account state captured at one point in a test.
///
/// Derefs to the decoded value, and also reports the address and lamport
/// balance it was read with.
pub struct Snapshot<T> {
    address: Pubkey,
    lamports: u64,
    state: T,
}

impl<T> Snapshot<T> {
    /// Address from which the state was read.
    pub fn address(&self) -> Pubkey {
        self.address
    }

    /// Lamport balance captured with the state.
    pub fn lamports(&self) -> u64 {
        self.lamports
    }
}

impl<T> Deref for Snapshot<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
