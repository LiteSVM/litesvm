use {
    solana_account::{
        Account, AccountSharedData, InheritableAccountFields, DUMMY_INHERITABLE_ACCOUNT_FIELDS,
    },
    solana_hash::Hash,
    solana_instructions_sysvar::construct_instructions_data,
    solana_message::SanitizedMessage,
    solana_sha256_hasher::Hasher,
    solana_transaction_error::TransactionError,
};

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

pub fn construct_instructions_account(
    message: &SanitizedMessage,
) -> Result<AccountSharedData, TransactionError> {
    // Fails when serialized instruction offsets exceed u16::MAX; LiteSVM does
    // not enforce the packet size limit, so oversized transactions can get here.
    let data = construct_instructions_data(&message.decompile_instructions())
        .map_err(|_| TransactionError::SanitizeFailure)?;
    Ok(AccountSharedData::from(Account {
        data,
        owner: solana_sdk_ids::sysvar::id(),
        ..Account::default()
    }))
}

pub(crate) fn create_loadable_account_with_fields(
    name: &str,
    (lamports, rent_epoch): InheritableAccountFields,
) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports,
        owner: solana_sdk_ids::native_loader::id(),
        data: name.as_bytes().to_vec(),
        executable: true,
        rent_epoch,
    })
}

pub(crate) fn create_loadable_account_for_test(name: &str) -> AccountSharedData {
    create_loadable_account_with_fields(name, DUMMY_INHERITABLE_ACCOUNT_FIELDS)
}
