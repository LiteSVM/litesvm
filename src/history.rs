use crate::types::TransactionResult;
use indexmap::IndexMap;
use solana_sdk::signature::Signature;

pub struct TransactionHistory(IndexMap<Signature, TransactionResult>);

impl TransactionHistory {
    pub fn new() -> Self {
        TransactionHistory(IndexMap::with_capacity(500))
    }

    pub fn set_capacity(&mut self, new_cap: usize) {
        if new_cap <= self.0.capacity() {
            self.0.shrink_to(new_cap)
        } else {
            self.0.reserve(new_cap - self.0.capacity())
        }
    }

    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionResult> {
        self.0.get(signature)
    }

    pub fn add_new_transaction(&mut self, signature: Signature, result: TransactionResult) {
        let capacity = self.0.capacity();
        if capacity != 0 {
            if self.0.len() == capacity {
                self.0.shift_remove_index(0);
            }
            self.0.insert(signature, result);
        }
    }

    pub fn check_transaction(&self, signature: &Signature) -> bool {
        self.0.contains_key(signature)
    }
}
