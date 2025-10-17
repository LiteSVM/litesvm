use serde::{Deserialize, Serialize};
use crate::merkle::hash::{AccountState, hash_leaf, hash_internal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    pub account_index: u32,
    pub account_state: AccountState,
    pub proof: Vec<[u8; 32]>,
    pub root: [u8; 32],
}

impl MerkleProof {
    pub fn verify(&self) -> bool {
        self.verify_with_height(self.proof.len() as u32)
    }

    pub fn verify_with_height(&self, expected_height: u32) -> bool {
        if self.proof.len() != expected_height as usize {
            return false;
        }

        if self.account_index >= (1 << expected_height) {
            return false;
        }

        let mut current_hash = hash_leaf(&self.account_state);
        let mut current_index = self.account_index;

        for sibling_hash in &self.proof {
            if current_index % 2 == 0 {
                current_hash = hash_internal(&current_hash, sibling_hash);
            } else {
                current_hash = hash_internal(sibling_hash, &current_hash);
            }
            current_index >>= 1;
        }

        current_hash == self.root
    }

    pub fn is_valid_bounds(&self, max_accounts: u32) -> bool {
        self.account_index < max_accounts
    }
}