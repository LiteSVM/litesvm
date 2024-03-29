use solana_sdk::hash::{Hash, Hasher};

pub mod loader;
pub mod rent;

/// Create a blockhash from the given bytes
pub fn create_blockhash(bytes: &[u8]) -> Hash {
    let mut hasher = Hasher::default();
    hasher.hash(bytes);
    hasher.result()
}
