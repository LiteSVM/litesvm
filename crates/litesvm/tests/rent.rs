use {
    agave_feature_set::{
        deprecate_rent_exemption_threshold, set_lamports_per_byte_to_5080,
        set_lamports_per_byte_to_6333, set_lamports_per_byte_to_696, set_lamports_per_byte_to_6960,
        FeatureSet,
    },
    litesvm::LiteSVM,
    solana_rent::{Rent, DEFAULT_LAMPORTS_PER_BYTE},
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
    feature_set.activate(&set_lamports_per_byte_to_696::id(), 0);
    assert_eq!(
        rent_with(feature_set).lamports_per_byte,
        set_lamports_per_byte_to_696::LAMPORTS_PER_BYTE
    );
}

#[test]
fn reset_safeguard_overrides_the_reductions() {
    let mut feature_set = LiteSVM::mainnet_feature_set();
    feature_set.activate(&set_lamports_per_byte_to_6960::id(), 0);
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
