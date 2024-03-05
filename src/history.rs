use crate::types::TransactionMetadata;
use solana_sdk::signature::Signature;
use std::collections::HashMap;

pub struct TransactionHistory(HashMap<Signature, TransactionMetadata>);

impl TransactionHistory {
    pub fn new() -> Self {
        TransactionHistory(HashMap::with_capacity(500))
    }

    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionMetadata> {
        self.0.get(signature)
    }

    pub fn add_new_transaction(&mut self, signature: Signature, meta: TransactionMetadata) {
        if self.0.len() == 500 {
            let key = *self.0.iter().next().unwrap().0;
            self.0.remove(&key);
        }
        self.0.insert(signature, meta);
    }

    pub fn check_transaction(&self, signature: &Signature) -> bool {
        self.0.contains_key(signature)
    }
}
