use {
    solana_account::{Account, AccountSharedData},
    solana_hash::{Hash, Hasher},
    solana_message::SanitizedMessage,
    solana_sdk_ids::sysvar,
    solana_sysvar::instructions::construct_instructions_data,
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
        owner: solana_sysvar::id(),
        ..Account::default()
    })
}
