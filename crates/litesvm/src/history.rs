use {crate::types::TransactionResult, indexmap::IndexMap, jupnet_sdk::signature::TypedSignature};

#[derive(Clone)]
pub struct TransactionHistory(IndexMap<TypedSignature, TransactionResult>);

impl TransactionHistory {
    pub fn new() -> Self {
        TransactionHistory(IndexMap::with_capacity(32))
    }

    pub fn set_capacity(&mut self, new_cap: usize) {
        if new_cap <= self.0.capacity() {
            self.0.truncate(new_cap);
            self.0.shrink_to_fit();
        } else {
            self.0.reserve(new_cap - self.0.capacity())
        }
    }

    pub fn get_transaction(&self, signature: &TypedSignature) -> Option<&TransactionResult> {
        self.0.get(signature)
    }

    pub fn add_new_transaction(&mut self, signature: TypedSignature, result: TransactionResult) {
        let capacity = self.0.capacity();
        if capacity != 0 {
            if self.0.len() == capacity {
                self.0.shift_remove_index(0);
            }
            self.0.insert(signature, result);
        }
    }

    pub fn check_transaction(&self, signature: &TypedSignature) -> bool {
        self.0.contains_key(signature)
    }
}
