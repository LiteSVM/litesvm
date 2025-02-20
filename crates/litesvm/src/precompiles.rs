use {
    solana_account::{AccountSharedData, WritableAccount},
    solana_precompiles::get_precompiles,
    solana_sdk_ids::native_loader,
};

use crate::LiteSVM;

pub(crate) fn load_precompiles(svm: &mut LiteSVM) {
    let mut account = AccountSharedData::default();
    account.set_owner(native_loader::id());
    account.set_lamports(1);
    account.set_executable(true);

    for precompile in get_precompiles() {
        if precompile
            .feature
            .map_or(true, |feature_id| svm.feature_set.is_active(&feature_id))
        {
            svm.set_account(precompile.program_id, account.clone().into())
                .unwrap();
        }
    }
}
