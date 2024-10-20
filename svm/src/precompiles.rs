use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    ed25519_program, native_loader, secp256k1_program,
};

use crate::LiteSVM;

pub(crate) fn load_precompiles(svm: &mut LiteSVM) {
    let mut account = AccountSharedData::default();
    account.set_owner(native_loader::id());
    account.set_lamports(1);
    account.set_executable(true);

    svm.set_account(ed25519_program::ID, account.clone().into())
        .unwrap();
    svm.set_account(secp256k1_program::ID, account.clone().into())
        .unwrap();
}
