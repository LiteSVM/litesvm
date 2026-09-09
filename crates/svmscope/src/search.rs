//! Counterfactual search — discover the smallest change that flips a replay's
//! outcome, instead of making the user guess mutations by hand.
//!
//! The core here is a monotonic boundary search over a numeric knob: "at what
//! oracle price does this liquidation stop succeeding?", "what is the minimum
//! balance that avoids the revert?". [`Replay::find_threshold`](crate::Replay)
//! wires a concrete mutation to it; this module is the pure, testable search.

use {crate::error::Result, serde::Serialize};

/// The result of a counterfactual threshold search: the value at which the
/// transaction's outcome flips, and the outcomes at the search bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Threshold {
    /// The boundary — the lowest value in `[lo, hi]` whose outcome differs from
    /// the outcome at `lo`. The flip happens between `flips_at - 1` and this.
    pub flips_at: u64,
    /// Whether the transaction succeeded at the low bound.
    pub low_success: bool,
    /// Whether the transaction succeeded at the high bound.
    pub high_success: bool,
    /// How many candidate values the search actually evaluated (replays run).
    pub evaluations: u32,
}

/// Binary-search the inclusive range `[lo, hi]` for the boundary where `ok`
/// flips. `ok(v)` reports whether the transaction succeeds at candidate `v`.
///
/// Returns `None` when the outcome is identical at both bounds (no flip in
/// range). Assumes a single crossing (the outcome is monotonic in the knob) —
/// the usual shape of a threshold like a balance, price, or deadline; with more
/// than one crossing it still returns *a* boundary, just not necessarily all.
pub(crate) fn search_threshold<F>(lo: u64, hi: u64, ok: F) -> Result<Option<Threshold>>
where
    F: Fn(u64) -> Result<bool>,
{
    assert!(lo <= hi, "search bounds inverted");
    let mut evaluations = 0u32;
    let mut eval = |v: u64| -> Result<bool> {
        evaluations += 1;
        ok(v)
    };

    let low_success = eval(lo)?;
    let high_success = eval(hi)?;
    if low_success == high_success {
        return Ok(None);
    }

    // Invariant: outcome(lo) == low_success, outcome(hi) == high_success, and
    // they differ. Narrow until adjacent; `hi` is then the first flipped value.
    let (mut lo, mut hi) = (lo, hi);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if eval(mid)? == low_success {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    Ok(Some(Threshold {
        flips_at: hi,
        low_success,
        high_success,
        evaluations,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_exact_boundary_of_a_monotonic_flip() {
        // Succeeds at/below 40, fails from 41 up.
        let th = search_threshold(0, 1000, |v| Ok(v <= 40))
            .unwrap()
            .expect("a flip exists in range");
        assert_eq!(th.flips_at, 41);
        assert!(th.low_success);
        assert!(!th.high_success);
        // log2(1000) ≈ 10, plus the two bounds — nowhere near a linear scan.
        assert!(th.evaluations < 16, "took {} evals", th.evaluations);
    }

    #[test]
    fn no_flip_when_both_bounds_agree() {
        assert!(search_threshold(0, 100, |_| Ok(true)).unwrap().is_none());
        assert!(search_threshold(0, 100, |_| Ok(false)).unwrap().is_none());
    }

    #[test]
    fn finds_a_boundary_that_flips_the_other_direction() {
        // Fails below 500, succeeds at/above it.
        let th = search_threshold(0, 4096, |v| Ok(v >= 500))
            .unwrap()
            .unwrap();
        assert_eq!(th.flips_at, 500);
        assert!(!th.low_success);
        assert!(th.high_success);
    }
}
