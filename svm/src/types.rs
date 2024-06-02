use solana_sdk::{
    account::AccountSharedData,
    instruction::InstructionError,
    program_error::ProgramError,
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Result, TransactionError},
    transaction_context::TransactionReturnData,
};

#[derive(Debug, Default, Clone)]
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

impl From<ProgramError> for FailedTransactionMetadata {
    fn from(value: ProgramError) -> Self {
        FailedTransactionMetadata {
            err: TransactionError::InstructionError(
                0,
                InstructionError::Custom(u64::from(value) as u32),
            ),
            meta: Default::default(),
        }
    }
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

impl ExecutionResult {
    pub(crate) fn result_and_compute_units(
        tx_result: Result<()>,
        compute_units_consumed: u64,
    ) -> Self {
        Self {
            tx_result,
            compute_units_consumed,
            ..Default::default()
        }
    }
}
