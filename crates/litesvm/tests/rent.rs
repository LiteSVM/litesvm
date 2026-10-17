use {
    agave_feature_set::{
        deprecate_rent_exemption_threshold, set_lamports_per_byte_to_1322,
        set_lamports_per_byte_to_2575, set_lamports_per_byte_to_5080,
        set_lamports_per_byte_to_6333, set_lamports_per_byte_to_696, set_lamports_per_byte_to_6960,
        FeatureSet,
    },
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::Address,
    solana_rent::{Rent, DEFAULT_LAMPORTS_PER_BYTE},
    solana_sdk_ids::sysvar,
};

fn rent_with(feature_set: FeatureSet) -> Rent {
    LiteSVM::default()
        .with_feature_set(feature_set)
        .with_sysvars()
        .get_sysvar::<Rent>()
}

#[test]
fn mainnet_feature_set_applies_the_latest_rent_reduction() {
    let rent = LiteSVM::new().get_sysvar::<Rent>();
    assert_eq!(
        rent.lamports_per_byte,
        set_lamports_per_byte_to_5080::LAMPORTS_PER_BYTE
    );
    assert_eq!(rent.minimum_balance(123), 251 * 5080);
}

#[test]
fn earlier_rent_reduction_applies_when_later_ones_are_inactive() {
    let mut feature_set = LiteSVM::mainnet_feature_set();
    feature_set.deactivate(&set_lamports_per_byte_to_5080::id());
    assert_eq!(
        rent_with(feature_set).lamports_per_byte,
        set_lamports_per_byte_to_6333::LAMPORTS_PER_BYTE
    );
}

#[test]
fn no_rent_reduction_keeps_the_default_value() {
    let mut feature_set = LiteSVM::mainnet_feature_set();
    feature_set.deactivate(&set_lamports_per_byte_to_6333::id());
    feature_set.deactivate(&set_lamports_per_byte_to_5080::id());
    assert_eq!(
        rent_with(feature_set).lamports_per_byte,
        DEFAULT_LAMPORTS_PER_BYTE
    );
}

#[test]
fn later_rent_reductions_can_be_activated() {
    let mut feature_set = LiteSVM::mainnet_feature_set();
    for (feature_id, lamports_per_byte) in [
        (set_lamports_per_byte_to_2575::id(), 2575),
        (set_lamports_per_byte_to_1322::id(), 1322),
        (set_lamports_per_byte_to_696::id(), 696),
    ] {
        feature_set.activate(&feature_id, 0);
        let rent = rent_with(feature_set.clone());
        assert_eq!(rent.lamports_per_byte, lamports_per_byte);
        assert_eq!(rent.minimum_balance(123), 251 * lamports_per_byte);
    }
}

#[test]
fn reset_safeguard_overrides_the_reductions() {
    let feature_set = FeatureSet::all_enabled();
    assert_eq!(
        rent_with(feature_set).lamports_per_byte,
        set_lamports_per_byte_to_6960::LAMPORTS_PER_BYTE
    );
}

#[test]
fn legacy_rent_representation_without_the_threshold_feature() {
    let mut feature_set = LiteSVM::mainnet_feature_set();
    feature_set.deactivate(&deprecate_rent_exemption_threshold::id());
    feature_set.deactivate(&set_lamports_per_byte_to_6333::id());
    feature_set.deactivate(&set_lamports_per_byte_to_5080::id());
    let rent = rent_with(feature_set);
    assert_eq!(rent.lamports_per_byte, DEFAULT_LAMPORTS_PER_BYTE / 2);
    assert_eq!(
        rent.minimum_balance(123),
        Rent::default().minimum_balance(123)
    );
}

#[test]
fn rejected_rent_write_keeps_the_cached_rent() {
    let mut svm = LiteSVM::new();
    let min_balance = svm.minimum_balance_for_rent_exemption(0);
    let invalid_rent = Account {
        lamports: 1,
        data: vec![0; 3],
        owner: sysvar::id(),
        executable: false,
        rent_epoch: 0,
    };

    assert!(svm.set_account(sysvar::rent::id(), invalid_rent).is_err());
    assert_eq!(svm.minimum_balance_for_rent_exemption(0), min_balance);
    svm.airdrop(&Address::new_unique(), min_balance).unwrap();
}
