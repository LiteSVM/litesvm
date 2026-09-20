//! Composable fixtures for common Solana accounts and programs.

use {
    crate::fixture::{
        accounts, Account, Ctx, Pubkey, SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
    },
    std::path::PathBuf,
};

/// State that can install itself into a test world.
///
/// Applications can implement this trait for protocol-level fixtures and
/// compose the built-in account fixtures inside [`Fixture::install`]. Arrays
/// of one fixture type are fixtures too, so repeated setup can be installed
/// with `test.add((Wallet::account(), Wallet::account()))`. Each fixture returns the address it
/// placed, so tests thread those handles instead of pinning addresses up front.
pub trait Fixture {
    /// Handle or state returned after installation.
    type Output;

    /// Install the fixture and return the handles needed by the test.
    fn install(self, test: &mut Ctx) -> Self::Output;
}

/// Closures are fixtures: `|t: &mut Ctx| { ... }` installs by running, and
/// its return value is the output — the dependency mechanism for worlds where
/// later fixtures need earlier handles. A world function returns one:
///
/// ```rust,ignore
/// fn escrow_setup(funding: u64) -> impl Fixture<Output = EscrowSetup> {
///     move |t: &mut Ctx| { /* add fixtures, register invariants */ }
/// }
/// ```
impl<O, F: FnOnce(&mut Ctx) -> O> Fixture for F {
    type Output = O;

    fn install(self, test: &mut Ctx) -> O {
        self(test)
    }
}

macro_rules! impl_fixture_for_tuple {
    ($($name:ident),+) => {
        /// Tuples are fixtures: one `add` installs a heterogeneous world, in
        /// order, and destructures its handles.
        impl<$($name: Fixture),+> Fixture for ($($name,)+) {
            type Output = ($($name::Output,)+);

            fn install(self, test: &mut Ctx) -> Self::Output {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                ($($name.install(test),)+)
            }
        }
    };
}

impl_fixture_for_tuple!(A, B);
impl_fixture_for_tuple!(A, B, C);
impl_fixture_for_tuple!(A, B, C, D);
impl_fixture_for_tuple!(A, B, C, D, E);

impl<F: Fixture, const N: usize> Fixture for [F; N] {
    type Output = [F::Output; N];

    fn install(self, test: &mut Ctx) -> Self::Output {
        self.map(|fixture| fixture.install(test))
    }
}

impl Fixture for Account {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        let address = self.address;
        test.set_account(self);
        address
    }
}

/// A system-owned, funded account, optionally holding token balances —
/// `Wallet::account().holding(mint, 1_000)` installs the wallet and a funded
/// associated token account per holding, the actor-centric dual of
/// [`Mint::with_holder`].
#[derive(Debug, Clone)]
pub struct Wallet {
    address: Option<Pubkey>,
    lamports: u64,
    holdings: Vec<(Pubkey, u64)>,
}

impl Wallet {
    /// One wallet with [`crate::fixture::DEFAULT_WALLET_LAMPORTS`] at the world's next
    /// deterministic address.
    pub fn account() -> Self {
        Self {
            address: None,
            lamports: crate::fixture::DEFAULT_WALLET_LAMPORTS,
            holdings: Vec::new(),
        }
    }

    /// Hold `amount` of `mint` through the wallet's associated token account,
    /// installed alongside the wallet. Repeatable for several mints.
    pub fn holding(mut self, mint: Pubkey, amount: u64) -> Self {
        self.holdings.push((mint, amount));
        self
    }

    /// Use a specific address instead of the world's next deterministic one.
    pub fn at(mut self, address: Pubkey) -> Self {
        self.address = Some(address);
        self
    }

    /// Fund the wallet with an exact lamport balance, replacing the default.
    pub fn fund(mut self, lamports: u64) -> Self {
        self.lamports = lamports;
        self
    }
}

impl Fixture for Wallet {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        let address = self.address.unwrap_or_else(|| test.fresh_address());
        test.set_account(accounts::system_account(address, self.lamports));
        for (mint, amount) in self.holdings {
            test.set_account(accounts::associated_token_account_with_program(
                address,
                mint,
                amount,
                crate::fixture::SPL_TOKEN_PROGRAM_ID,
            ));
        }
        address
    }
}

/// Which token program owns a mint or token account fixture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TokenProgram {
    /// The original SPL Token program.
    #[default]
    Legacy,
    /// The Token-2022 program.
    Token2022,
}

impl TokenProgram {
    pub(crate) fn id(self) -> Pubkey {
        match self {
            Self::Legacy => SPL_TOKEN_PROGRAM_ID,
            Self::Token2022 => SPL_TOKEN_2022_PROGRAM_ID,
        }
    }
}

/// An initialized token mint.
///
/// A bare [`Mint::account`] has no mint authority, modelling a fixed-supply mint.
/// [`Self::with_authority`] makes it mintable and [`Self::with_freeze_authority`]
/// installs a freeze authority. [`Self::with_holder`] additionally installs one
/// associated-token account per holder, so a funded mint and its holders can be
/// set up in a single fixture.
#[derive(Debug, Clone)]
pub struct Mint {
    authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
    supply: u64,
    decimals: u8,
    token_program: TokenProgram,
    holders: Vec<(Pubkey, u64)>,
}

impl Mint {
    /// One six-decimal legacy Token mint with zero supply and no mint or
    /// freeze authority, at the world's next deterministic address. Without
    /// [`Self::with_authority`] the mint is fixed-supply (its `mint_authority`
    /// is `COption::None`). Installing the mint returns its address, so tests
    /// read that back rather than pinning it.
    pub fn account() -> Self {
        Self {
            authority: None,
            freeze_authority: None,
            supply: 0,
            decimals: 6,
            token_program: TokenProgram::Legacy,
            holders: Vec::new(),
        }
    }

    /// Set the mint authority, making the mint mintable. Left unset, the mint
    /// is fixed-supply.
    pub fn with_authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the freeze authority. Left unset, the mint cannot freeze accounts.
    pub fn with_freeze_authority(mut self, freeze_authority: Pubkey) -> Self {
        self.freeze_authority = Some(freeze_authority);
        self
    }

    /// Set the initial token supply.
    pub fn with_supply(mut self, supply: u64) -> Self {
        self.supply = supply;
        self
    }

    /// Set the mint precision.
    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    /// Select the token program that owns the mint.
    pub fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = token_program;
        self
    }

    /// Fund each `(owner, amount)` holder with `amount` of this mint through its
    /// associated-token account, created when the mint is installed.
    ///
    /// Takes the whole set in one call, from anything iterable (an array,
    /// slice, or vec). Reach for [`AssociatedTokenAccount`] or [`TokenAccount`]
    /// directly when a holder needs a non-associated address or other explicit
    /// control.
    pub fn with_holder(mut self, holders: impl IntoIterator<Item = (Pubkey, u64)>) -> Self {
        self.holders.extend(holders);
        self
    }
}

impl Fixture for Mint {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        let address = test.fresh_address();
        test.set_account(accounts::token_program_mint_account(
            address,
            self.authority,
            self.freeze_authority,
            self.supply,
            self.decimals,
            self.token_program.id(),
        ));
        for (owner, amount) in self.holders {
            test.set_account(accounts::associated_token_account_with_program(
                owner,
                address,
                amount,
                self.token_program.id(),
            ));
        }
        address
    }
}

/// An initialized token account at an arbitrary address.
#[derive(Debug, Clone, Copy)]
pub struct TokenAccount {
    address: Option<Pubkey>,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    token_program: TokenProgram,
}

impl TokenAccount {
    /// One empty legacy Token account for `mint`, owned by `owner`, at the
    /// world's next deterministic address.
    pub fn account(mint: Pubkey, owner: Pubkey) -> Self {
        Self {
            address: None,
            mint,
            owner,
            amount: 0,
            token_program: TokenProgram::Legacy,
        }
    }

    /// Install the token account at a specific address.
    pub fn at(mut self, address: Pubkey) -> Self {
        self.address = Some(address);
        self
    }

    /// Set the initial token balance.
    pub fn with_amount(mut self, amount: u64) -> Self {
        self.amount = amount;
        self
    }

    /// Select the token program that owns the account.
    pub fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = token_program;
        self
    }
}

impl Fixture for TokenAccount {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        let address = self.address.unwrap_or_else(|| test.fresh_address());
        test.set_account(accounts::token_program_account(
            address,
            self.mint,
            self.owner,
            self.amount,
            self.token_program.id(),
        ));
        address
    }
}

/// An initialized token account at its associated-token address.
///
/// Deliberately has no `accounts` plural: an associated token address is a
/// pure function of its owner and mint, so "several ATAs" only means several
/// owners — and funding several owners of one mint is [`Mint::with_holder`]'s
/// job.
#[derive(Debug, Clone, Copy)]
pub struct AssociatedTokenAccount {
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    token_program: TokenProgram,
}

impl AssociatedTokenAccount {
    /// One empty legacy associated-token account for `mint` and `owner`, at
    /// their derived associated-token address.
    pub fn account(mint: Pubkey, owner: Pubkey) -> Self {
        Self {
            mint,
            owner,
            amount: 0,
            token_program: TokenProgram::Legacy,
        }
    }

    /// Set the initial token balance.
    pub fn with_amount(mut self, amount: u64) -> Self {
        self.amount = amount;
        self
    }

    /// Select the token program used in address derivation and ownership.
    pub fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = token_program;
        self
    }
}

impl Fixture for AssociatedTokenAccount {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        let account = accounts::associated_token_account_with_program(
            self.owner,
            self.mint,
            self.amount,
            self.token_program.id(),
        );
        let address = account.address;
        test.set_account(account);
        address
    }
}

/// Copy real accounts (or a real program) from a live cluster into the world.
///
/// `Dump` fills a committed `.parallax/` store next to the consuming project's
/// manifest, so the first run fetches once and every later run is fully offline
/// and deterministic. The RPC endpoint comes from
/// [`Ctx::builder(id).rpc(url)`](crate::fixture::CtxBuilder::rpc); unset, it defaults to
/// the public mainnet-beta RPC.
///
/// ```rust,ignore
/// use litesvm::fixture::prelude::*;
///
/// #[parallax_test]
/// fn against_real_state(test: &mut Ctx) {
///     // Fetched once at one slot, then served from `.parallax/` offline.
///     let [pool, oracle] = test.add(Dump::accounts([POOL, ORACLE]));
///     test.add(Dump::program(AMM_PROGRAM));
///     test.execute(SwapInstruction { pool, oracle }).succeeds();
/// }
/// ```
///
/// [`Dump::accounts`] returns the same addresses it was given, in the same
/// arity, so a test destructures them directly (`let [a, b] = ...`).
/// [`Dump::program`] dumps a program's executable account and its loader-v3
/// programdata coherently and loads it as a usable program.
/// [`DumpAccounts::sync_clock`]/[`DumpProgram::sync_clock`] opt the world into
/// adopting the dumped slot's clock. [`Dump::refresh_all`] re-fetches every
/// stored entry in one coherent batch.
pub struct Dump;

impl Dump {
    /// Dump a fixed set of accounts. Returns the same addresses, so the test
    /// reads them straight back: `let [a, b] = test.add(Dump::accounts([A, B]));`.
    pub fn accounts<const N: usize>(addresses: [Pubkey; N]) -> DumpAccounts<N> {
        DumpAccounts {
            addresses,
            sync_clock: false,
        }
    }

    /// Dump a program (its executable account and, for the upgradeable loader,
    /// its programdata) and load it as a usable program. Returns the program id.
    pub fn program(program_id: Pubkey) -> DumpProgram {
        DumpProgram {
            program_id,
            sync_clock: false,
        }
    }

    /// Re-fetch every entry already in the store in one coherent batch, refresh
    /// their slots, and reinstall them. Returns the refreshed addresses. Use it
    /// to collapse a world whose dumped entries drifted across many slots back
    /// onto a single recent slot.
    pub fn refresh_all() -> DumpRefresh {
        DumpRefresh
    }
}

/// The [`Dump::accounts`] fixture: a fixed-arity set of addresses to dump.
pub struct DumpAccounts<const N: usize> {
    addresses: [Pubkey; N],
    sync_clock: bool,
}

impl<const N: usize> DumpAccounts<N> {
    /// Adopt the dumped slot's clock (its slot and a timestamp derived from it)
    /// for this world. Opt-in; off by default.
    pub fn sync_clock(mut self) -> Self {
        self.sync_clock = true;
        self
    }
}

impl<const N: usize> Fixture for DumpAccounts<N> {
    type Output = [Pubkey; N];

    fn install(self, test: &mut Ctx) -> Self::Output {
        test.dump_accounts_native(&self.addresses, self.sync_clock);
        self.addresses
    }
}

/// The [`Dump::program`] fixture: a program to dump and load.
pub struct DumpProgram {
    program_id: Pubkey,
    sync_clock: bool,
}

impl DumpProgram {
    /// Adopt the dumped slot's clock for this world. Opt-in; off by default.
    pub fn sync_clock(mut self) -> Self {
        self.sync_clock = true;
        self
    }
}

impl Fixture for DumpProgram {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        test.dump_program_native(self.program_id, self.sync_clock);
        self.program_id
    }
}

/// The [`Dump::refresh_all`] fixture: re-fetch every stored entry at one slot.
pub struct DumpRefresh;

impl Fixture for DumpRefresh {
    type Output = Vec<Pubkey>;

    fn install(self, test: &mut Ctx) -> Self::Output {
        test.refresh_all_native()
    }
}

/// Install accounts (or a program) from an already-dumped file at an explicit
/// path — no store, no network, ever.
///
/// A dump file is the same wincode format the `.parallax/` store writes, so
/// `Load` reads any file [`Dump`] produced: copy a file out of `.parallax/`
/// (or commit it anywhere), and `Load` it by path to share fixtures across
/// tests, machines, and languages.
///
/// ```rust,ignore
/// use litesvm::fixture::prelude::*;
///
/// #[parallax_test]
/// fn against_a_shared_dump(test: &mut Ctx) {
///     let accounts = test.add(Load::accounts("fixtures/pool.dump"));
///     test.add(Load::program("fixtures/amm.dump"));
///     // ...
/// }
/// ```
///
/// [`Load::accounts`] returns every installed address; [`Load::program`] loads
/// the program and returns its id.
pub struct Load;

impl Load {
    /// Install every account in the dump file at `path`. Returns the installed
    /// addresses (arity is the file's, known at runtime, so this is a `Vec`).
    pub fn accounts(path: impl Into<PathBuf>) -> LoadAccounts {
        LoadAccounts { path: path.into() }
    }

    /// Load the program in the dump file at `path` as a usable program. Returns
    /// the program id.
    pub fn program(path: impl Into<PathBuf>) -> LoadProgram {
        LoadProgram { path: path.into() }
    }
}

/// The [`Load::accounts`] fixture.
pub struct LoadAccounts {
    path: PathBuf,
}

impl Fixture for LoadAccounts {
    type Output = Vec<Pubkey>;

    fn install(self, test: &mut Ctx) -> Self::Output {
        test.load_file_native(&self.path, false)
    }
}

/// The [`Load::program`] fixture.
pub struct LoadProgram {
    path: PathBuf,
}

impl Fixture for LoadProgram {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        test.load_file_native(&self.path, true)
            .into_iter()
            .next()
            .expect("a program file loads exactly one program id")
    }
}

/// A program to preload for cross-program invocations.
pub struct Program<'a> {
    id: Pubkey,
    elf: &'a [u8],
}

impl<'a> Program<'a> {
    /// Create a program fixture from its address and compiled ELF bytes.
    pub fn new(id: Pubkey, elf: &'a [u8]) -> Self {
        Self { id, elf }
    }
}

impl Fixture for Program<'_> {
    type Output = Pubkey;

    fn install(self, test: &mut Ctx) -> Self::Output {
        test.preload_program(self.id, self.elf);
        self.id
    }
}
