use jupnet_sdk::{
    account::{
        Account, AccountSharedData, InheritableAccountFields, DUMMY_INHERITABLE_ACCOUNT_FIELDS,
    },
    hash::{Hash, Hasher},
    message::SanitizedMessage,
    native_loader,
    sysvar::{self, instructions::construct_instructions_data},
};

pub mod builtins;
pub mod inner_instructions;
pub mod rent;
#[cfg(feature = "serde")]
pub mod serde_with_str;

/// Create a blockhash from the given bytes
pub fn create_blockhash(bytes: &[u8]) -> Hash {
    let mut hasher = Hasher::default();
    hasher.hash(bytes);
    hasher.result()
}

pub fn construct_instructions_account(message: &SanitizedMessage) -> AccountSharedData {
    let data = message
        .to_legacy_message()
        .map(|legacy| construct_instructions_data(&legacy.decompile_instructions()))
        .unwrap_or_default();
    AccountSharedData::from(Account {
        data,
        owner: sysvar::id(),
        ..Account::default()
    })
}
pub(crate) fn create_loadable_account_with_fields(
    name: &str,
    (lamports, rent_epoch): InheritableAccountFields,
) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports,
        owner: native_loader::id(),
        data: name.as_bytes().to_vec(),
        executable: true,
        rent_epoch,
    })
}

pub(crate) fn create_loadable_account_for_test(name: &str) -> AccountSharedData {
    create_loadable_account_with_fields(name, DUMMY_INHERITABLE_ACCOUNT_FIELDS)
}
