use litesvm_fixture::{parallax_test, Pubkey};

#[parallax_test(program_id = Pubkey::new_from_array([1; 32]))]
#[ignore = "compile-only direct crate path check"]
fn macro_resolves_fixture_crate() {}
