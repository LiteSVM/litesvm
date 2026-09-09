//! The assertion DSL: declare what a replay should do, mollusk-style.
//!
//! ```no_run
//! use svmscope::{Check, Cmp, Scenario, Mutation, Scope};
//!
//! let scope = Scope::new("https://api.mainnet-beta.solana.com");
//! let replay = scope.replay("<signature>")?;
//! let outcomes = replay.run_suite(&[
//!     Scenario::new("baseline replays faithfully")
//!         .check(Check::success()),
//!     Scenario::new("draining the vault reverts")
//!         .mutate(Mutation::lamports("<vault>", 0))
//!         .check(Check::revert_contains("InsufficientFunds"))
//!         .check(Check::account("<user>").token_delta(Cmp::eq(0)).build()),
//! ])?;
//! assert!(outcomes.iter().all(|o| o.pass));
//! # Ok::<(), svmscope::Error>(())
//! ```

use crate::{
    analyze::ReplayResult,
    mutation::{AccountAssert, CmpOp, Expect, Mutation, StateCheck},
};

/// A comparison against a numeric value: `Cmp::eq(5)`, `Cmp::ge(-100)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cmp {
    pub(crate) op: CmpOp,
    pub(crate) value: i128,
}

macro_rules! cmp_ctor {
    ($name:ident, $op:ident, $doc:literal) => {
        #[doc = $doc]
        pub fn $name(value: impl Into<i128>) -> Cmp {
            Cmp {
                op: CmpOp::$op,
                value: value.into(),
            }
        }
    };
}

impl Cmp {
    cmp_ctor!(eq, Eq, "`actual == value`");
    cmp_ctor!(ne, Ne, "`actual != value`");
    cmp_ctor!(lt, Lt, "`actual < value`");
    cmp_ctor!(le, Le, "`actual <= value`");
    cmp_ctor!(gt, Gt, "`actual > value`");
    cmp_ctor!(ge, Ge, "`actual >= value`");
}

/// One declarative check on a replay's outcome or resulting state.
/// Construct via the associated functions; combine on a [`Scenario`].
#[derive(Debug, Clone)]
pub struct Check(pub(crate) CheckKind);

#[derive(Debug, Clone)]
pub(crate) enum CheckKind {
    /// Transaction-level outcome (success / revert / revert containing text).
    Outcome(Expect),
    /// The error or a log line contains this text (regardless of outcome).
    LogContains(String),
    /// Compute units consumed satisfy the comparison.
    ComputeUnits(Cmp),
    /// The local outcome (success/failure) matches the recorded on-chain one.
    MatchesOnchain,
    /// Post-replay state checks on one account.
    Account(Vec<AccountAssert>),
}

impl Check {
    /// The transaction must succeed. (Also the default: a scenario with no
    /// outcome check implicitly asserts success.)
    pub fn success() -> Check {
        Check(CheckKind::Outcome(Expect::Success))
    }

    /// The transaction must fail (any error).
    pub fn revert() -> Check {
        Check(CheckKind::Outcome(Expect::Revert))
    }

    /// The transaction must fail *and* the error or a log line contains this
    /// text (an error name, `Custom(6025)`, a message…).
    pub fn revert_contains(text: impl Into<String>) -> Check {
        Check(CheckKind::Outcome(Expect::RevertContains(text.into())))
    }

    /// No outcome assertion — opt out of the implicit [`Check::success`].
    pub fn any_outcome() -> Check {
        Check(CheckKind::Outcome(Expect::Any))
    }

    /// A log line (or the error) contains this text.
    pub fn log_contains(text: impl Into<String>) -> Check {
        Check(CheckKind::LogContains(text.into()))
    }

    /// The local replay's outcome matches what actually happened on-chain —
    /// the regression check for frozen fixtures. Needs a recorded outcome
    /// ([`crate::Replay::recorded`]); fails with an explanation otherwise.
    pub fn matches_onchain() -> Check {
        Check(CheckKind::MatchesOnchain)
    }

    /// Compute units consumed satisfy the comparison —
    /// `Check::compute_units(Cmp::le(200_000))`.
    pub fn compute_units(cmp: Cmp) -> Check {
        Check(CheckKind::ComputeUnits(cmp))
    }

    /// Start a builder of state checks on one account:
    /// `Check::account(pda).field("count", Cmp::eq(100)).build()`.
    pub fn account(address: impl Into<String>) -> AccountCheck {
        AccountCheck {
            address: address.into(),
            asserts: Vec::new(),
        }
    }
}

/// Builder for state checks against one account, finished with
/// [`AccountCheck::build`]. All checks read post-replay state; `*_delta`
/// variants compare against the pre-transaction value.
#[derive(Debug)]
pub struct AccountCheck {
    address: String,
    asserts: Vec<AccountAssert>,
}

impl AccountCheck {
    fn push(mut self, check: StateCheck) -> Self {
        self.asserts.push(AccountAssert {
            address: self.address.clone(),
            check,
        });
        self
    }

    /// The account's lamports satisfy the comparison.
    pub fn lamports(self, c: Cmp) -> Self {
        self.push(StateCheck::Lamports {
            op: c.op,
            value: c.value,
        })
    }

    /// The change in lamports (post − pre) satisfies the comparison.
    pub fn lamports_delta(self, c: Cmp) -> Self {
        self.push(StateCheck::LamportsDelta {
            op: c.op,
            value: c.value,
        })
    }

    /// The SPL token amount (u64 at offset 64) satisfies the comparison.
    pub fn token_amount(self, c: Cmp) -> Self {
        self.push(StateCheck::U64At {
            offset: 64,
            op: c.op,
            value: c.value,
        })
    }

    /// The change in SPL token amount (post − pre) satisfies the comparison.
    pub fn token_delta(self, c: Cmp) -> Self {
        self.push(StateCheck::TokenDelta {
            op: c.op,
            value: c.value,
        })
    }

    /// The little-endian u64 at `offset` satisfies the comparison.
    pub fn u64_at(self, offset: usize, c: Cmp) -> Self {
        self.push(StateCheck::U64At {
            offset,
            op: c.op,
            value: c.value,
        })
    }

    /// The named field of the account's decoded layout satisfies the
    /// comparison — `pool.reserveA` instead of `u64@72`. Resolves against
    /// built-in layouts or the owner program's IDL; integer/bool fields only.
    pub fn field(self, name: impl Into<String>, c: Cmp) -> Self {
        self.push(StateCheck::Field {
            name: name.into(),
            op: c.op,
            value: c.value,
        })
    }

    /// The change in the named field (post − pre) satisfies the comparison.
    pub fn field_delta(self, name: impl Into<String>, c: Cmp) -> Self {
        self.push(StateCheck::FieldDelta {
            name: name.into(),
            op: c.op,
            value: c.value,
        })
    }
    /// The named field is byte-for-byte identical before and after the
    /// transaction. Unlike [`field_delta`](Self::field_delta) this works for
    /// every field type — a pubkey authority, an optional delegate, a flag.
    pub fn field_unchanged(self, name: impl Into<String>) -> Self {
        self.push(StateCheck::FieldUnchanged { name: name.into() })
    }

    /// Finish the builder.
    pub fn build(self) -> Check {
        Check(CheckKind::Account(self.asserts))
    }
}

/// One named test case: what-if mutations plus the checks that must hold.
/// A scenario with no outcome check implicitly asserts [`Check::success`].
#[derive(Debug, Clone)]
pub struct Scenario {
    /// The scenario's name, shown in outcomes.
    pub name: String,
    /// Mutations applied to the world before replaying.
    pub mutations: Vec<Mutation>,
    /// Checks that must hold after the replay.
    pub checks: Vec<Check>,
}

impl Scenario {
    /// A scenario with no mutations and no checks yet.
    pub fn new(name: impl Into<String>) -> Scenario {
        Scenario {
            name: name.into(),
            mutations: Vec::new(),
            checks: Vec::new(),
        }
    }

    /// Add a what-if mutation, applied before the replay.
    pub fn mutate(mut self, m: Mutation) -> Scenario {
        self.mutations.push(m);
        self
    }

    /// Add a check that must hold after the replay.
    pub fn check(mut self, c: Check) -> Scenario {
        self.checks.push(c);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_expands_to_one_assert_per_constraint() {
        let Check(CheckKind::Account(asserts)) = Check::account("X")
            .lamports(Cmp::ge(1))
            .field("count", Cmp::eq(100))
            .build()
        else {
            panic!("expected account checks")
        };
        assert_eq!(asserts.len(), 2);
        assert_eq!(asserts[0].address, "X");
    }

    #[test]
    fn scenario_builder_accumulates() {
        let s = Scenario::new("drain")
            .mutate(Mutation::lamports("V", 0))
            .check(Check::revert())
            .check(Check::account("U").token_delta(Cmp::eq(0)).build());
        assert_eq!(s.name, "drain");
        assert_eq!(s.mutations.len(), 1);
        assert_eq!(s.checks.len(), 2);
    }
}

/// The result of one post-replay assertion.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssertOutcome {
    /// Human description of what was asserted.
    pub description: String,
    /// Whether the assertion held.
    pub pass: bool,
}

/// The result of running one scenario.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ScenarioOutcome {
    /// The scenario's name.
    pub name: String,
    /// Human description of the transaction-level expectation.
    pub expect: String,
    /// Whether everything asserted (outcome + all state checks) held.
    pub pass: bool,
    /// What the replay actually did.
    pub actual: ReplayResult,
    /// Each state assertion's individual result.
    pub asserts: Vec<AssertOutcome>,
}
