use {crate::types::TransactionResult, indexmap::IndexMap, solana_signature::Signature};

#[derive(Clone)]
pub struct TransactionHistory {
    entries: IndexMap<Signature, TransactionResult>,
    capacity: usize,
}

impl TransactionHistory {
    pub fn new() -> Self {
        Self {
            entries: IndexMap::with_capacity(32),
            capacity: 32,
        }
    }

    pub fn set_capacity(&mut self, new_cap: usize) {
        if new_cap <= self.capacity {
            self.entries.truncate(new_cap);
            self.entries.shrink_to_fit();
        }
        self.capacity = new_cap;
    }

    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionResult> {
        self.entries.get(signature)
    }

    pub fn is_enabled(&self) -> bool {
        self.capacity != 0
    }

    pub fn add_new_transaction(&mut self, signature: Signature, result: TransactionResult) {
        let capacity = self.capacity;
        if capacity != 0 {
            if self.entries.len() == capacity {
                self.entries.shift_remove_index(0);
            }
            self.entries.insert(signature, result);
        }
    }

    pub fn check_transaction(&self, signature: &Signature) -> bool {
        self.entries.contains_key(signature)
    }

    #[cfg(feature = "persistence-internal")]
    pub fn entries(&self) -> &IndexMap<Signature, TransactionResult> {
        &self.entries
    }

    #[cfg(feature = "persistence-internal")]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[cfg(feature = "persistence-internal")]
    pub fn from_entries(entries: IndexMap<Signature, TransactionResult>, capacity: usize) -> Self {
        let mut history = Self { entries, capacity };
        history.set_capacity(capacity);
        history
    }
}
