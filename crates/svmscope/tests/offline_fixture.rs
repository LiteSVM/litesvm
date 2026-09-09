//! Integration test: the crate exactly as a consumer uses it, fully offline.
//!
//! The committed fixture froze a real devnet transaction (an Anchor counter
//! program's `increment_counter`) with its accounts, program ELF, on-chain
//! IDL, and recorded outcome. Everything below runs with zero RPC.

use svmscope::{Check, Cmp, Fixture, Mutation, Replay, Scenario};

const FIXTURE: &str = include_str!("fixtures/counter_increment.json");
const COUNTER_PDA: &str = "FLFUsWEUjARKvDrFf9WtF7k1tCxHqtCpfJU2krL6z7vY";

/// A frozen pre-cliff vesting claim that reverted on-chain (Anchor error 6003).
/// Guards `matches_onchain` fidelity for a *reverting* recorded outcome offline.
const REVERT_FIXTURE: &str = include_str!("fixtures/vesting_precliff_revert.json");

fn replay() -> Replay {
    Replay::from_fixture(&Fixture::from_json(FIXTURE).expect("fixture parses"))
        .expect("fixture loads")
}

#[test]
fn frozen_replay_matches_the_onchain_outcome() {
    let out = replay()
        .verify("faithful", &[], &[Check::matches_onchain()])
        .unwrap();
    assert!(out.pass, "{out:?}");
}

#[test]
fn named_field_asserts_resolve_offline_via_the_captured_idl() {
    // The fixture froze the counter at 1 (state at capture time, not at the
    // tx's original slot — replays run against reconstructed current state),
    // so the increment lands on 2. The delta is the robust assertion.
    let out = replay()
        .verify(
            "count increments",
            &[],
            &[Check::account(COUNTER_PDA)
                .field("count", Cmp::eq(2))
                .field_delta("count", Cmp::eq(1))
                .build()],
        )
        .unwrap();
    assert!(out.pass, "{out:?}");
}

#[test]
fn mutations_and_the_check_dsl_compose() {
    // Patch count to 99 pre-replay; the increment lands on 100.
    let out = replay()
        .verify(
            "patched counter reaches 100",
            &[Mutation::patch(
                COUNTER_PDA,
                8,
                99u64.to_le_bytes().to_vec(),
            )],
            &[
                Check::success(),
                Check::log_contains("incremented by 1 to : 100"),
                Check::account(COUNTER_PDA)
                    .field("count", Cmp::eq(100))
                    .build(),
                Check::compute_units(Cmp::le(200_000)),
            ],
        )
        .unwrap();
    assert!(out.pass, "{out:?}");
}

#[test]
fn named_field_mutations_mirror_named_field_asserts() {
    // The same what-if as the byte-patch test above, but by NAME: no offsets.
    // Set count to 99 pre-replay; the increment lands on 100.
    let out = replay()
        .verify(
            "field mutation reaches 100",
            &[Mutation::field(COUNTER_PDA, "count", 99)],
            &[
                Check::success(),
                Check::account(COUNTER_PDA)
                    .field("count", Cmp::eq(100))
                    .build(),
            ],
        )
        .unwrap();
    assert!(out.pass, "{out:?}");
}

#[test]
fn field_mutation_errors_are_hard_and_specific() {
    // Unknown field: a hard error naming the fields that do exist — never a
    // silently ignored mutation.
    let err = replay()
        .verify(
            "typo'd field",
            &[Mutation::field(COUNTER_PDA, "countt", 1)],
            &[Check::success()],
        )
        .unwrap_err();
    assert!(
        matches!(&err, svmscope::Error::UnknownField { available, .. } if available.iter().any(|f| f == "count")),
        "{err}"
    );

    // Out-of-range value for the field's type: a hard error, not a truncation.
    let err = replay()
        .verify(
            "negative into u64",
            &[Mutation::field(COUNTER_PDA, "count", -1)],
            &[Check::success()],
        )
        .unwrap_err();
    assert!(
        matches!(err, svmscope::Error::FieldValueOutOfRange { .. }),
        "{err}"
    );
}

#[test]
fn time_travel_warps_the_clock() {
    let mut replay = replay();
    let before = replay.describe_clock();
    replay.advance_seconds(30 * 86_400);
    let after = replay.describe_clock();
    assert_ne!(before, after);
    // The counter has no time gate — it still succeeds in the future.
    assert!(replay.run().unwrap().result.success);
}

#[test]
fn a_typoed_mutation_address_is_a_hard_error_not_a_passing_revert() {
    let typo = "7NypoTypoTypoTypoTypoTypoTypoTypoTypoTypoTyp";
    let err = replay()
        .run_suite(&[Scenario::new("drain")
            .mutate(Mutation::lamports(typo, 0))
            .check(Check::revert())])
        .unwrap_err();
    assert!(
        matches!(
            err,
            svmscope::Error::InvalidAddress(_) | svmscope::Error::MutationTargetMissing(_)
        ),
        "{err}"
    );
}

#[test]
fn matches_onchain_alone_holds_for_a_reverting_recorded_outcome() {
    // The recorded outcome here is a revert. `matches_onchain` on its own must
    // pass when the offline replay also reverts — it must NOT be sabotaged by the
    // implicit "expect success" default that applies when no outcome check is
    // present. (Regression: previously this failed because the implicit success
    // expectation contradicted the faithful revert.)
    let replay = Replay::from_fixture(&Fixture::from_json(REVERT_FIXTURE).unwrap()).unwrap();
    let alone = replay
        .verify("matches on-chain", &[], &[Check::matches_onchain()])
        .unwrap();
    assert!(
        alone.pass,
        "matches_onchain alone should pass on a revert: {alone:?}"
    );

    // And pairing it with an explicit revert() still passes.
    let paired = replay
        .verify(
            "revert + match",
            &[],
            &[Check::revert(), Check::matches_onchain()],
        )
        .unwrap();
    assert!(paired.pass, "{paired:?}");

    // Sanity: the recorded outcome really is a failure.
    assert!(replay.recorded().is_some_and(|r| !r.success));
}

#[test]
fn a_typoed_assert_address_fails_the_check_instead_of_silently_passing() {
    // A `lamports == 0` / `token_delta == 0` assertion against an address the
    // replay never loaded used to read a silent zero and PASS. It must now fail
    // the assertion (with the account named), the same no-silent-pass guarantee
    // the mutation path gives. "So1111…112" is a valid address absent here.
    let absent = "So11111111111111111111111111111111111111112";
    let out = replay()
        .verify(
            "typo'd assert address",
            &[],
            &[
                Check::account(absent).lamports(Cmp::eq(0)).build(),
                Check::account(absent).token_delta(Cmp::eq(0)).build(),
            ],
        )
        .unwrap();
    assert!(!out.pass, "{out:?}");
    assert!(
        out.asserts.iter().all(|a| !a.pass),
        "both asserts must fail, not vacuously pass: {out:?}"
    );
}

#[test]
fn unknown_fields_error_with_the_available_names() {
    let out = replay()
        .verify(
            "bad field name",
            &[],
            &[Check::account(COUNTER_PDA)
                .field("countt", Cmp::eq(1))
                .build()],
        )
        .unwrap();
    // A failed field resolution is a failed assertion whose description names
    // the available fields, so the fix is right in the test output.
    assert!(!out.pass);
    assert!(out.asserts[0].description.contains("count"), "{out:?}");
}

#[test]
fn delta_asserts_measure_from_the_mutated_start_not_the_original_load() {
    // The counter is loaded at 1. A mutation sets it to 99 before the run and
    // the program increments it to 100. The transaction's own effect is +1;
    // a delta of +99 would be crediting the mutation to the program.
    let mutation = Mutation::patch(COUNTER_PDA, 8, 99u64.to_le_bytes().to_vec());
    let real = replay()
        .verify(
            "delta is the increment",
            std::slice::from_ref(&mutation),
            &[Check::account(COUNTER_PDA)
                .field_delta("count", Cmp::eq(1))
                .build()],
        )
        .unwrap();
    assert!(real.pass, "{real:?}");
    let wrong = replay()
        .verify(
            "delta must not include the mutation",
            std::slice::from_ref(&mutation),
            &[Check::account(COUNTER_PDA)
                .field_delta("count", Cmp::eq(99))
                .build()],
        )
        .unwrap();
    assert!(
        !wrong.pass,
        "a +99 delta credits the mutation to the program: {wrong:?}"
    );
}
