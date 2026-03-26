use {
    jupnet_sdk::feature::{self as feature_gate, Feature},
    litesvm::LiteSVM,
};

#[test_log::test]
fn new_initializes_accounts_for_enabled_features() {
    let svm = LiteSVM::new();
    let feature_id = jupnet_feature_set::add_new_reserved_account_keys::id();

    let account = svm
        .get_account(&feature_id)
        .expect("active feature account should exist");
    let feature = feature_gate::from_account(&account).expect("feature account should deserialize");

    assert_eq!(account.owner, jupnet_sdk::feature::id());
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
