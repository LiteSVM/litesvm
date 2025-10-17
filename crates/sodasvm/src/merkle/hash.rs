use sha3::{Digest, Keccak256};
use solana_pubkey::Pubkey;

const LEAF_PREFIX: u8 = 0x00;
const INTERNAL_PREFIX: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccountState {
    pub pubkey: Pubkey,
    pub balance: u64,
    pub nonce: u64,
    pub last_update: i64,
}

impl AccountState {
    pub fn new(pubkey: Pubkey, balance: u64, nonce: u64, last_update: i64) -> Self {
        Self {
            pubkey,
            balance,
            nonce,
            last_update,
        }
    }
}

pub fn hash_leaf(account: &AccountState) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(&[LEAF_PREFIX]);
    hasher.update(account.pubkey.to_bytes());
    hasher.update(account.balance.to_le_bytes());
    hasher.update(account.nonce.to_le_bytes());
    hasher.update(account.last_update.to_le_bytes());
    hasher.finalize().into()
}

pub fn hash_internal(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(&[INTERNAL_PREFIX]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

pub fn hash_empty() -> [u8; 32] {
    [0; 32]
}