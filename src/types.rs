use solana_sdk::{
    account::AccountSharedData,
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Result, TransactionError},
    transaction_context::TransactionReturnData,
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct TransactionMetadata {
    pub signature: Signature,
    pub logs: Vec<String>,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
}

#[derive(Debug, Clone)]
pub struct FailedTransactionMetadata {
    pub err: TransactionError,
    pub meta: TransactionMetadata,
}

pub type TransactionResult = std::result::Result<TransactionMetadata, FailedTransactionMetadata>;

pub(crate) struct ExecutionResult {
    pub(crate) post_accounts: Vec<(Pubkey, AccountSharedData)>,
    pub(crate) tx_result: Result<()>,
    pub(crate) signature: Signature,
    pub(crate) compute_units_consumed: u64,
    pub(crate) return_data: TransactionReturnData,
    /// Whether the transaction can be included in a block
    pub(crate) included: bool,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            post_accounts: Default::default(),
            tx_result: Err(TransactionError::UnsupportedVersion),
            signature: Default::default(),
            compute_units_consumed: Default::default(),
            return_data: Default::default(),
            included: false,
        }
    }
}

#[derive(Error, Debug)]
pub enum InvalidSysvarDataError {
    #[error("Invalid Clock sysvar data.")]
    Clock,
    #[error("Invalid EpochRewards sysvar data.")]
    EpochRewards,
    #[error("Invalid EpochSchedule sysvar data.")]
    EpochSchedule,
    #[error("Invalid Fees sysvar data.")]
    Fees,
    #[error("Invalid LastRestartSlot sysvar data.")]
    LastRestartSlot,
    #[error("Invalid RecentBlockhashes sysvar data.")]
    RecentBlockhashes,
    #[error("Invalid Rent sysvar data.")]
    Rent,
    #[error("Invalid SlotHashes sysvar data.")]
    SlotHashes,
    #[error("Invalid StakeHistory sysvar data.")]
    StakeHistory,
}

#[derive(Error, Debug)]
pub enum LiteSVMError {
    #[error("{0}")]
    InvalidSysvarData(#[from] InvalidSysvarDataError),
    #[error("{0}")]
    Instruction(#[from] InstructionError),
}
