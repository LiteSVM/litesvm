use litesvm::fixture::{parallax_test, Pubkey};

const ID: Pubkey = Pubkey::new_from_array([42; 32]);

#[parallax_test]
#[ignore = "compile-only macro path check"]
fn macro_resolves_fixture_module() {
    let _ = ctx;
}
