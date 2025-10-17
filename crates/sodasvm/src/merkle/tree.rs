use std::collections::HashMap;
use crate::merkle::hash::{AccountState, hash_leaf, hash_internal, hash_empty};
use crate::merkle::proof::MerkleProof;

#[derive(Debug, Clone)]
pub struct SodaMerkleTree {
    pub root: [u8; 32],
    pub height: u32,
    pub leaves: Vec<AccountState>,
    pub nodes: HashMap<(u32, u32), [u8; 32]>,
}

impl SodaMerkleTree {
    pub fn new(accounts: Vec<AccountState>) -> Self {
        let mut tree = Self {
            root: hash_empty(),
            height: 0,
            leaves: accounts,
            nodes: HashMap::new(),
        };
        tree.build();
        tree
    }

    fn build(&mut self) {
        if self.leaves.is_empty() {
            return;
        }

        let leaf_count = self.leaves.len();
        self.height = (leaf_count as f64).log2().ceil() as u32;
        let tree_size = 1 << self.height;

        for (i, account) in self.leaves.iter().enumerate() {
            self.nodes.insert((0, i as u32), hash_leaf(account));
        }

        let empty_hash = hash_empty();
        for i in leaf_count..tree_size {
            self.nodes.insert((0, i as u32), empty_hash);
        }

        for level in 1..=self.height {
            let level_size = tree_size >> level;
            for i in 0..level_size {
                let left_child = self.nodes.get(&(level - 1, (i * 2) as u32)).unwrap();
                let right_child = self.nodes.get(&(level - 1, (i * 2 + 1) as u32)).unwrap();

                let parent_hash = hash_internal(left_child, right_child);
                self.nodes.insert((level, i as u32), parent_hash);
            }
        }

        self.root = self.nodes.get(&(self.height, 0)).copied().unwrap_or(hash_empty());
    }

    pub fn generate_proof(&self, account_index: usize) -> Option<MerkleProof> {
        if account_index >= self.leaves.len() {
            return None;
        }

        let mut proof = Vec::new();
        let mut current_index = account_index as u32;

        for level in 0..self.height {
            let sibling_index = current_index ^ 1;
            if let Some(sibling_hash) = self.nodes.get(&(level, sibling_index)) {
                proof.push(*sibling_hash);
            }
            current_index >>= 1;
        }

        Some(MerkleProof {
            account_index: account_index as u32,
            account_state: self.leaves[account_index],
            proof,
            root: self.root,
        })
    }

    pub fn update_account(&mut self, index: usize, new_state: AccountState) -> Result<(), String> {
        if index >= self.leaves.len() {
            return Err("Account index out of bounds".to_string());
        }

        self.leaves[index] = new_state;
        self.update_path(index);
        Ok(())
    }

    fn update_path(&mut self, leaf_index: usize) {
        let mut current_index = leaf_index as u32;

        self.nodes.insert((0, current_index), hash_leaf(&self.leaves[leaf_index]));

        for level in 1..=self.height {
            let parent_index = current_index >> 1;
            let left_child = self.nodes.get(&(level - 1, parent_index * 2)).unwrap();
            let right_child = self.nodes.get(&(level - 1, parent_index * 2 + 1)).unwrap();

            let parent_hash = hash_internal(left_child, right_child);
            self.nodes.insert((level, parent_index), parent_hash);

            current_index = parent_index;
        }

        self.root = self.nodes.get(&(self.height, 0)).copied().unwrap_or(hash_empty());
    }

    pub fn add_account(&mut self, account: AccountState) {
        self.leaves.push(account);
        self.build();
    }
}