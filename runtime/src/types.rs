use solana_sdk::{transaction::Result, transaction_context::TransactionReturnData};

#[derive(Debug)]
pub struct TransactionMetadata {
    pub logs: Vec<String>,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
}

#[derive(Debug)]
pub struct TransactionResult {
    pub result: Result<()>,
    pub metadata: TransactionMetadata,
}
