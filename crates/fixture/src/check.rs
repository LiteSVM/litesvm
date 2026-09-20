//! Check values: the verification half of the harness.
//!
//! The verdict methods ([`succeeds`](crate::Outcome::succeeds) /
//! [`fails`](crate::Outcome::fails) / [`fails_with`](crate::Outcome::fails_with))
//! produce a transaction witness; everything asserted about that transaction is
//! a [`CheckFn`] — built from the fact namespaces, a [`bundle`] of other
//! checks, or [`CheckFn::new`] over the whole transaction. Every fact takes its
//! subject and one *expectation*: a plain value means equality, a closure is a
//! predicate over the measured value:
//!
//! ```rust,ignore
//! test.execute(deposit).succeeds().checks([
//!     Cu::spent(|cu| cu <= 5_000),
//!     Account::lamports(vault, amount),                 // value ⇒ equality
//!     Account::lamports(signer, |x| x > 0),             // predicate ⇒ anything
//!     Account::data(vault, |v: &Vault| v.amount > 0),   // decoded through T's schema
//!     Account::created(vault),
//! ]);
//! ```
//!
//! Value expectations fail with `expected N, got M`; predicates cannot print
//! their source, so they fail with the location of the line that built them.

use {
    crate::{
        outcome::{mint_supply, token_amount},
        Outcome, ProgramError, Pubkey,
    },
    core::panic::Location,
    wincode::{config::DefaultConfig, SchemaRead},
};

/// One assertion against a succeeded transaction.
///
/// Leaf facts, [`bundle`]s, and ad-hoc closures all reduce to this one type,
/// so they group freely in the same [`checks`](Outcome::checks)
/// array and nest arbitrarily. A panic in any leaf unwinds through the
/// containing bundles.
pub struct CheckFn(Box<dyn Fn(&Outcome)>);

impl CheckFn {
    /// Wrap a closure over the whole transaction — the escape hatch for
    /// anything the fact namespaces cannot spell (relations across accounts,
    /// log scans, ...). Assert or panic inside; returning means the check
    /// passed.
    pub fn new(check: impl Fn(&Outcome) + 'static) -> Self {
        Self(Box::new(check))
    }

    /// Run the check against `tx`, panicking when it fails.
    pub fn run(&self, tx: &Outcome) {
        (self.0)(tx)
    }
}

/// Convert several checks into one check. A bundle is itself a [`CheckFn`],
/// so bundles nest, and a protocol's standard assertions become a plain
/// function returning one:
///
/// ```rust,ignore
/// fn vault_invariants(vault: Pubkey, program: Pubkey) -> CheckFn {
///     bundle([
///         Account::owner(vault, program),
///         Account::data(vault, |v: &Vault| v.amount > 0),
///     ])
/// }
/// ```
pub fn bundle(checks: impl IntoIterator<Item = CheckFn>) -> CheckFn {
    let checks: Vec<_> = checks.into_iter().collect();
    CheckFn::new(move |tx| {
        for check in &checks {
            check.run(tx);
        }
    })
}

/// An expectation over a measured value of type `T`: a plain `T` means
/// equality (and prints itself on failure), a `Fn(T) -> bool` closure is an
/// arbitrary predicate (and fails with the location that built it).
pub trait Expected<T> {
    /// Whether the expectation holds for `actual`.
    fn holds(&self, actual: &T) -> bool;
    /// The expected value, when it can be printed (`None` for predicates).
    fn describe(&self) -> Option<String>;
}

impl Expected<u64> for u64 {
    fn holds(&self, actual: &u64) -> bool {
        actual == self
    }

    fn describe(&self) -> Option<String> {
        Some(self.to_string())
    }
}

impl<F: Fn(u64) -> bool> Expected<u64> for F {
    fn holds(&self, actual: &u64) -> bool {
        self(*actual)
    }

    fn describe(&self) -> Option<String> {
        None
    }
}

impl Expected<Pubkey> for Pubkey {
    fn holds(&self, actual: &Pubkey) -> bool {
        actual == self
    }

    fn describe(&self) -> Option<String> {
        Some(self.to_string())
    }
}

impl<F: Fn(Pubkey) -> bool> Expected<Pubkey> for F {
    fn holds(&self, actual: &Pubkey) -> bool {
        self(*actual)
    }

    fn describe(&self) -> Option<String> {
        None
    }
}

/// An expectation over raw bytes: exact bytes, or a predicate over the slice.
pub trait ExpectedBytes {
    /// Whether the expectation holds for `actual`.
    fn holds(&self, actual: &[u8]) -> bool;
    /// The expected bytes, when they can be printed (`None` for predicates).
    fn describe(&self) -> Option<String>;
}

fn render_bytes(bytes: &[u8]) -> String {
    format!("{bytes:?}")
}

impl<const N: usize> ExpectedBytes for [u8; N] {
    fn holds(&self, actual: &[u8]) -> bool {
        actual == self
    }

    fn describe(&self) -> Option<String> {
        Some(render_bytes(self))
    }
}

impl ExpectedBytes for Vec<u8> {
    fn holds(&self, actual: &[u8]) -> bool {
        actual == self.as_slice()
    }

    fn describe(&self) -> Option<String> {
        Some(render_bytes(self))
    }
}

impl ExpectedBytes for &'static [u8] {
    fn holds(&self, actual: &[u8]) -> bool {
        actual == *self
    }

    fn describe(&self) -> Option<String> {
        Some(render_bytes(self))
    }
}

impl<F: Fn(&[u8]) -> bool> ExpectedBytes for F {
    fn holds(&self, actual: &[u8]) -> bool {
        self(actual)
    }

    fn describe(&self) -> Option<String> {
        None
    }
}

/// Panic for a failed expectation: `expected N, got M` when the expectation
/// prints itself, else the predicate's construction site and the actual value.
fn fail(
    label: &str,
    expected: Option<String>,
    actual: &dyn core::fmt::Display,
    location: &Location,
) -> ! {
    match expected {
        Some(expected) => panic!("{label}: expected {expected}, got {actual}"),
        None => panic!("{label}: predicate at {location} failed — actual: {actual}"),
    }
}

fn value_fact<T: core::fmt::Display + 'static>(
    label: String,
    location: &'static Location<'static>,
    read: impl Fn(&Outcome) -> T + 'static,
    expected: impl Expected<T> + 'static,
) -> CheckFn {
    CheckFn::new(move |tx| {
        live(tx);
        let actual = read(tx);
        if !expected.holds(&actual) {
            fail(&label, expected.describe(), &actual, location);
        }
    })
}

/// Facts self-diagnose: a fact evaluated against a failed transaction panics
/// leading with the transaction's error and logs — otherwise it would silently
/// judge pre-state, since a failed transaction commits nothing.
fn live(tx: &Outcome) {
    if let Some(error) = tx.failure() {
        panic!(
            "fact checked against a failed transaction: {error}{}",
            tx.formatted_logs()
        );
    }
}

fn required(tx: &Outcome, address: Pubkey) -> &crate::Account {
    live(tx);
    tx.account(address)
        .unwrap_or_else(|| panic!("transaction does not contain account {address}"))
}

fn change(tx: &Outcome, address: Pubkey) -> &crate::AccountChange {
    live(tx);
    tx.account_changes()
        .iter()
        .find(|change| change.address() == address)
        .unwrap_or_else(|| panic!("this transaction did not change account {address}"))
}

/// Compute-unit facts: `Cu::spent(|cu| cu <= 5_000)`, `Cu::spent(1_556)`.
pub struct Cu;

impl Cu {
    /// The compute units the transaction consumed.
    #[track_caller]
    pub fn spent(expected: impl Expected<u64> + 'static) -> CheckFn {
        value_fact(
            "compute units".into(),
            Location::caller(),
            |tx| tx.compute_units(),
            expected,
        )
    }
}

/// The account-scoped facts, hung off the [`Account`](crate::Account) type
/// itself: one noun installs raw accounts and measures them.
impl crate::Account {
    /// The lamport balance of the account at `address`.
    #[track_caller]
    pub fn lamports(address: Pubkey, expected: impl Expected<u64> + 'static) -> CheckFn {
        value_fact(
            format!("lamports of {address}"),
            Location::caller(),
            move |tx| required(tx, address).lamports,
            expected,
        )
    }

    /// The owner of the account at `address`.
    #[track_caller]
    pub fn owner(address: Pubkey, expected: impl Expected<Pubkey> + 'static) -> CheckFn {
        value_fact(
            format!("owner of {address}"),
            Location::caller(),
            move |tx| required(tx, address).owner,
            expected,
        )
    }

    /// The data of the account at `address` — raw bytes when expected raw,
    /// decoded through `T`'s wincode schema (the same decode path as
    /// [`Ctx::read`](crate::Ctx::read)) when the predicate takes `&T`:
    ///
    /// ```rust,ignore
    /// Account::data(config, [1, 0, 0, 0]),                     // raw
    /// Account::data(vault, |v: &Vault| v.amount > 0),          // typed
    /// Account::data(vault, |v: &Vault| *v == Vault { .. }),    // typed equality
    /// ```
    ///
    /// Ownership is intentionally not checked; pair with [`Self::owner`] when
    /// it matters.
    #[track_caller]
    pub fn data<M>(address: Pubkey, expected: impl DataExpected<M> + 'static) -> CheckFn {
        let location = Location::caller();
        CheckFn::new(move |tx| {
            expected.verify(address, location, &required(tx, address).data);
        })
    }

    /// Assert the transaction created the account at `address` (absent
    /// before, present after).
    pub fn created(address: Pubkey) -> CheckFn {
        CheckFn::new(move |tx| {
            assert!(
                change(tx, address).was_created(),
                "account {address} was not created by this transaction"
            );
        })
    }

    /// Assert the transaction removed the account at `address` (present
    /// before, absent after).
    pub fn removed(address: Pubkey) -> CheckFn {
        CheckFn::new(move |tx| {
            assert!(
                change(tx, address).was_removed(),
                "account {address} was not removed by this transaction"
            );
        })
    }

    /// Assert Solana's closed-account state at `address`: the runtime may
    /// remove the account entirely or retain its empty system-owned form.
    pub fn closed(address: Pubkey) -> CheckFn {
        CheckFn::new(move |tx| {
            live(tx);
            if let Some(account) = tx.account(address) {
                assert_eq!(
                    account.lamports, 0,
                    "closed account {address} still holds lamports"
                );
                assert!(
                    account.data.is_empty(),
                    "closed account {address} still holds data"
                );
                assert_eq!(
                    account.owner,
                    crate::system_program::ID,
                    "closed account {address} is not system-owned"
                );
            }
        })
    }
}

/// An expectation over an account's data. Raw byte expectations compare the
/// bytes; a `Fn(&T) -> bool` predicate decodes through `T`'s wincode schema
/// first. The `M` marker only steers inference — call sites never name it.
pub trait DataExpected<M> {
    /// Verify the expectation against the account's raw data.
    fn verify(&self, address: Pubkey, location: &'static Location<'static>, data: &[u8]);
}

/// Marker for raw-byte data expectations.
pub struct Raw;

/// Marker for typed data predicates over `T`.
pub struct Typed<T>(core::marker::PhantomData<T>);

impl<E: ExpectedBytes> DataExpected<Raw> for E {
    fn verify(&self, address: Pubkey, location: &'static Location<'static>, data: &[u8]) {
        if !self.holds(data) {
            fail(
                &format!("data of {address}"),
                self.describe(),
                &render_bytes(data),
                location,
            );
        }
    }
}

impl<T, F> DataExpected<Typed<T>> for F
where
    T: for<'de> SchemaRead<'de, DefaultConfig, Dst = T> + core::fmt::Debug,
    F: Fn(&T) -> bool,
{
    fn verify(&self, address: Pubkey, location: &'static Location<'static>, data: &[u8]) {
        let actual = crate::world::decode::<T>("data", address, data, 0);
        if !self(&actual) {
            panic!("data of {address}: predicate at {location} failed — actual: {actual:#?}");
        }
    }
}

/// The mint fact, on the [`Mint`](crate::fixture::Mint) fixture type itself.
/// Reads Token or Token-2022 mints.
impl crate::fixture::Mint {
    /// The supply of the mint at `address`: `Mint::supply(mint, 1_000)`.
    #[track_caller]
    pub fn supply(address: Pubkey, expected: impl Expected<u64> + 'static) -> CheckFn {
        value_fact(
            format!("supply of {address}"),
            Location::caller(),
            move |tx| mint_supply(required(tx, address)),
            expected,
        )
    }
}

/// The token-account fact, on the
/// [`TokenAccount`](crate::fixture::TokenAccount) fixture type itself. Reads
/// Token or Token-2022 accounts.
impl crate::fixture::TokenAccount {
    /// The token balance of the token account at `address`:
    /// `TokenAccount::amount(ata, |x| x >= 500)`.
    #[track_caller]
    pub fn amount(address: Pubkey, expected: impl Expected<u64> + 'static) -> CheckFn {
        value_fact(
            format!("token balance of {address}"),
            Location::caller(),
            move |tx| token_amount(required(tx, address)),
            expected,
        )
    }
}

/// A transaction error expectation: a [`ProgramError`] compares directly, and
/// anything `Into<u32>` (a program's typed error) compares as
/// `ProgramError::Custom`.
pub trait IntoTransactionError {
    /// The [`ProgramError`] this expectation compares against.
    fn into_error(self) -> ProgramError;
}

impl IntoTransactionError for ProgramError {
    fn into_error(self) -> ProgramError {
        self
    }
}

impl<E: Into<u32>> IntoTransactionError for E {
    fn into_error(self) -> ProgramError {
        ProgramError::Custom(self.into())
    }
}

/// The verdict facts, hung off the [`Outcome`] type itself — one noun returns
/// the outcome and names its verdicts.
impl Outcome {
    /// Assert the transaction succeeded. Optional before other facts (they
    /// self-diagnose on a failed transaction), and useful to state intent:
    /// `ctx.execute(deposit).checks([Outcome::success(), ..])`.
    pub fn success() -> CheckFn {
        CheckFn::new(|tx| {
            if let Some(error) = tx.failure() {
                panic!("expected success, got {error}{}", tx.formatted_logs());
            }
        })
    }

    /// Assert the transaction failed with exactly this error — a
    /// [`ProgramError`], or a program's typed error (anything `Into<u32>`):
    /// `ctx.execute(forged).check(Outcome::error(VaultError::Unauthorized))`.
    pub fn error(expected: impl IntoTransactionError + 'static) -> CheckFn {
        let expected = expected.into_error();
        CheckFn::new(move |tx| {
            assert_eq!(
                tx.failure(),
                Some(&expected),
                "unexpected execution outcome{}",
                tx.formatted_logs()
            );
        })
    }
}

/// Transaction return-data facts: `ReturnData::is([7])`,
/// `ReturnData::is(|data: &[u8]| data.len() == 8)`.
pub struct ReturnData;

impl ReturnData {
    /// The transaction's return data.
    #[track_caller]
    pub fn is(expected: impl ExpectedBytes + 'static) -> CheckFn {
        let location = Location::caller();
        CheckFn::new(move |tx| {
            live(tx);
            let actual = tx.return_data();
            if !expected.holds(actual) {
                fail(
                    "return data",
                    expected.describe(),
                    &render_bytes(actual),
                    location,
                );
            }
        })
    }
}
