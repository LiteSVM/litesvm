use {
    litesvm::{features::MAINNET_ACTIVE_FEATURES, LiteSVM},
    solana_feature_gate_interface::{self as feature_gate, Feature},
};

#[test_log::test]
fn new_initializes_accounts_for_enabled_features() {
    let svm = LiteSVM::new();
    for (feature_id, activation_slot) in MAINNET_ACTIVE_FEATURES {
        let account = svm
            .get_account(feature_id)
            .unwrap_or_else(|| panic!("active feature account {feature_id} should exist"));
        let feature =
            feature_gate::from_account(&account).expect("feature account should deserialize");

        assert_eq!(account.owner, solana_sdk_ids::feature::id(), "{feature_id}");
        assert_eq!(
            feature,
            Feature {
                activated_at: Some(*activation_slot)
            },
            "{feature_id}"
        );
        assert!(
            account.lamports >= svm.minimum_balance_for_rent_exemption(Feature::size_of()),
            "feature account {feature_id} should be rent exempt"
        );
    }
}

#[test_log::test]
fn new_does_not_initialize_accounts_for_inactive_mainnet_features() {
    let svm = LiteSVM::new();
    for feature_id in LiteSVM::mainnet_feature_set().inactive() {
        assert!(
            svm.get_account(feature_id).is_none(),
            "inactive feature account {feature_id} should not exist"
        );
    }
}

#[test_log::test]
fn mainnet_feature_set_matches_mainnet_snapshot() {
    let feature_set = LiteSVM::mainnet_feature_set();

    assert_eq!(feature_set.active().len(), MAINNET_ACTIVE_FEATURES.len());
    for (feature_id, activation_slot) in MAINNET_ACTIVE_FEATURES {
        assert_eq!(
            feature_set.active().get(feature_id),
            Some(activation_slot),
            "{feature_id}"
        );
    }
}
