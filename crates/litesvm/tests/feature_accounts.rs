use {
    agave_feature_set::{accounts_lt_hash, alpenglow, raise_cpi_nesting_limit_to_8},
    litesvm::LiteSVM,
    solana_feature_gate_interface::{self as feature_gate, Feature},
};

#[test_log::test]
fn new_initializes_accounts_for_enabled_features() {
    let svm = LiteSVM::new();
    let feature_id = accounts_lt_hash::id();

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

#[test_log::test]
fn new_does_not_initialize_accounts_for_inactive_mainnet_features() {
    let svm = LiteSVM::new();
    assert!(
        svm.get_account(&raise_cpi_nesting_limit_to_8::id())
            .is_none(),
        "feature accounts should not be created for features inactive on mainnet"
    );
}

#[test_log::test]
fn mainnet_feature_set_matches_mainnet_activation_state() {
    let feature_set = LiteSVM::mainnet_feature_set();

    assert!(
        feature_set.is_active(&accounts_lt_hash::id()),
        "accounts_lt_hash is active on mainnet (SIMD-0215)"
    );
    assert!(
        !feature_set.is_active(&alpenglow::id()),
        "alpenglow is not yet active on mainnet"
    );
    assert!(
        !feature_set.is_active(&raise_cpi_nesting_limit_to_8::id()),
        "raise_cpi_nesting_limit_to_8 is not yet active on mainnet"
    );
}
