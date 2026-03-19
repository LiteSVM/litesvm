use {
    agave_feature_set::raise_cpi_nesting_limit_to_8,
    litesvm::LiteSVM,
    solana_feature_gate_interface::{self as feature_gate, Feature},
};

#[test_log::test]
fn new_initializes_accounts_for_enabled_features() {
    let svm = LiteSVM::new();
    let feature_id = raise_cpi_nesting_limit_to_8::id();

    let account = svm
        .get_account(&feature_id)
        .expect("active feature account should exist");
    let feature = feature_gate::from_account(&account).expect("feature account should deserialize");

    assert_eq!(account.owner, solana_sdk_ids::feature::id());
    assert_eq!(
        feature,
        Feature {
            activated_at: Some(0)
        }
    );
    assert!(
        account.lamports >= svm.minimum_balance_for_rent_exemption(Feature::size_of()),
        "feature account should be rent exempt"
    );
}
