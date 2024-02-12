use solana_sdk::{
    account::AccountSharedData,
    pubkey::Pubkey,
    transaction::{Result, TransactionError},
    transaction_context::TransactionReturnData,
};

#[derive(Debug)]
pub struct TransactionMetadata {
    pub logs: Vec<String>,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
}

#[derive(Debug)]
pub struct FailedTransactionMetadata {
    pub err: TransactionError,
    pub meta: TransactionMetadata,
}

pub type TransactionResult = std::result::Result<TransactionMetadata, FailedTransactionMetadata>;

pub(crate) struct ExecutionResult {
    pub post_accounts: Vec<(Pubkey, AccountSharedData)>,
    pub tx_result: Result<()>,
    pub programs_modified: Vec<Pubkey>,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            post_accounts: Default::default(),
            tx_result: Ok(()),
            programs_modified: Default::default(),
            compute_units_consumed: Default::default(),
            return_data: Default::default(),
        }
    }
}
