use solana_program_runtime::loaded_programs::LoadedProgramsForTxBatch;
use solana_sdk::{
    account::AccountSharedData, pubkey::Pubkey, transaction::Result,
    transaction_context::TransactionReturnData,
};

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

pub(crate) struct ExecutionResult {
    pub post_accounts: Vec<(Pubkey, AccountSharedData)>,
    pub tx_result: Result<()>,
    pub programs_modified: LoadedProgramsForTxBatch,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
}
