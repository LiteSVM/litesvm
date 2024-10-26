use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    feature_set::FeatureSet,
    native_loader,
    precompiles::get_precompiles,
};

use crate::LiteSVM;

pub(crate) fn load_precompiles(svm: &mut LiteSVM, feature_set: FeatureSet) {
    let mut account = AccountSharedData::default();
    account.set_owner(native_loader::id());
    account.set_lamports(1);
    account.set_executable(true);

    for precompile in get_precompiles() {
        if precompile
            .feature
            .map_or(true, |feature_id| feature_set.is_active(&feature_id))
        {
            svm.set_account(precompile.program_id, account.clone().into())
                .unwrap();
        }
    }
}
