use {
    solana_account::{
        Account, AccountSharedData, InheritableAccountFields, DUMMY_INHERITABLE_ACCOUNT_FIELDS,
    },
    solana_hash::Hash,
    solana_instructions_sysvar::construct_instructions_data,
    solana_message::SanitizedMessage,
    solana_sha256_hasher::Hasher,
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

pub fn construct_instructions_account(message: &SanitizedMessage) -> AccountSharedData {
    AccountSharedData::from(Account {
        data: construct_instructions_data(&message.decompile_instructions()),
        owner: solana_sdk_ids::sysvar::id(),
        ..Account::default()
    })
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

pub(crate) fn as_bytes<T: Copy>(slice: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice)) }
}

pub(crate) fn cast_slice(slice: &[u8]) -> Option<&[u64]> {
    let ptr = slice.as_ptr() as *const u64;
    if (slice.len() % size_of::<u64>()) != 0 || !ptr.is_aligned() {
        return None;
    }

    let len = slice.len() / size_of::<u64>();
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}
