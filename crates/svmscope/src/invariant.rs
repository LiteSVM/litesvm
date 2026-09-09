//! Invariant templates — named security properties, each compiled to the same
//! [`Check`] the scenario engine already runs. Instead of hand-assembling a
//! comparison, you assert a property by name ("this vault loses no tokens",
//! "this authority can't change"), and it composes with mutations, [`verify`],
//! and counterfactual search: search for the smallest change that *violates* an
//! invariant, not just one that flips success.
//!
//! [`verify`]: crate::Replay::verify
//!
//! ```
//! use svmscope::Invariant;
//! // The pool's reserve counter must never run backwards.
//! let inv = Invariant::monotonic("Poo1…", "reserve");
//! ```

use crate::check::{Check, Cmp};

/// A named security property over a replay's resulting state. Every constructor
/// returns a [`Check`]; drop it into a [`Scenario`](crate::Scenario) or
/// [`Replay::verify`](crate::Replay::verify) alongside your mutations.
pub struct Invariant;

impl Invariant {
    /// An account's authority (or any identity field) must not change — the
    /// classic account-takeover guard. `field` is resolved via the account's
    /// layout or owner IDL and compared byte-for-byte, so it works on the
    /// pubkey fields authorities actually are.
    pub fn authority_unchanged(account: impl Into<String>, field: impl Into<String>) -> Check {
        Check::account(account).field_unchanged(field).build()
    }

    /// An account's lamports must not decrease — no unexpected SOL outflow.
    pub fn no_lamport_loss(account: impl Into<String>) -> Check {
        Check::account(account).lamports_delta(Cmp::ge(0)).build()
    }

    /// An account may lose at most `max` lamports — a bounded-drain guard.
    pub fn max_lamport_loss(account: impl Into<String>, max: u64) -> Check {
        Check::account(account)
            .lamports_delta(Cmp::ge(-(max as i128)))
            .build()
    }

    /// An account's SPL token balance must not decrease — no token drain.
    pub fn no_token_loss(account: impl Into<String>) -> Check {
        Check::account(account).token_delta(Cmp::ge(0)).build()
    }

    /// An account may lose at most `max` raw token units.
    pub fn max_token_loss(account: impl Into<String>, max: u64) -> Check {
        Check::account(account)
            .token_delta(Cmp::ge(-(max as i128)))
            .build()
    }

    /// A counter field must be monotonic non-decreasing — never runs backwards.
    pub fn monotonic(account: impl Into<String>, field: impl Into<String>) -> Check {
        Check::account(account)
            .field_delta(field, Cmp::ge(0))
            .build()
    }

    /// A field must be unchanged by the transaction (a frozen config value).
    /// Any field type: compared byte-for-byte.
    pub fn field_constant(account: impl Into<String>, field: impl Into<String>) -> Check {
        Check::account(account).field_unchanged(field).build()
    }

    /// A field must hold an exact expected value after the transaction.
    pub fn field_equals(
        account: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<i128>,
    ) -> Check {
        Check::account(account).field(field, Cmp::eq(value)).build()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            check::CheckKind,
            mutation::{CmpOp, StateCheck},
        },
    };

    fn one_assert(check: &Check) -> (&str, &StateCheck) {
        match &check.0 {
            CheckKind::Account(asserts) if asserts.len() == 1 => {
                (asserts[0].address.as_str(), &asserts[0].check)
            }
            _ => panic!("expected a single-assert account check"),
        }
    }

    #[test]
    fn max_lamport_loss_is_a_bounded_negative_delta() {
        let (addr, sc) = {
            let c = Invariant::max_lamport_loss("Vau1t", 500);
            let (a, s) = one_assert(&c);
            (a.to_string(), s.clone())
        };
        assert_eq!(addr, "Vau1t");
        match sc {
            StateCheck::LamportsDelta { op, value } => {
                assert_eq!(op, CmpOp::Ge);
                assert_eq!(value, -500);
            }
            other => panic!("expected LamportsDelta, got {other:?}"),
        }
    }

    #[test]
    fn authority_unchanged_compares_the_field_bytes() {
        let c = Invariant::authority_unchanged("Mkt", "admin");
        let (_, sc) = one_assert(&c);
        match sc {
            StateCheck::FieldUnchanged { name } => assert_eq!(name, "admin"),
            other => panic!("expected FieldUnchanged, got {other:?}"),
        }
    }

    #[test]
    fn monotonic_requires_a_non_negative_field_delta() {
        let c = Invariant::monotonic("Pool", "reserve");
        let (_, sc) = one_assert(&c);
        match sc {
            StateCheck::FieldDelta { op, value, .. } => {
                assert_eq!(*op, CmpOp::Ge);
                assert_eq!(*value, 0);
            }
            other => panic!("expected FieldDelta, got {other:?}"),
        }
    }
}
