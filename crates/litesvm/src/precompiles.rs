use {
    crate::LiteSVM, agave_precompiles::get_precompiles, solana_account::Account,
    solana_sdk_ids::native_loader,
};

pub(crate) fn load_precompiles(svm: &mut LiteSVM) {
    let account = Account {
        lamports: 1,
        owner: native_loader::id(),
        executable: true,
        ..Default::default()
    };

    for precompile in get_precompiles() {
        if precompile
            .feature
            .is_none_or(|feature_id| svm.feature_set.is_active(&feature_id))
        {
            svm.set_account(precompile.program_id, account.clone())
                .unwrap();
        }
    }
}
