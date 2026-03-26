use {
    crate::format_logs::format_logs,
    jupnet_sdk::{
        account::AccountSharedData, inner_instruction::InnerInstructionsList,
        instruction::InstructionError, program_error::ProgramError, pubkey::Pubkey,
        signature::TypedSignature, transaction_context::TransactionReturnData,
    },
    jupnet_transaction_error::TransactionError,
};

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransactionMetadata {
    #[cfg_attr(feature = "serde", serde(with = "crate::utils::serde_with_str"))]
    pub signature: TypedSignature,
    pub logs: Vec<String>,
    pub inner_instructions: InnerInstructionsList,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
    pub fee: u64,
}

impl TransactionMetadata {
    pub fn pretty_logs(&self) -> String {
        format_logs(&self.logs)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SimulatedTransactionInfo {
    pub meta: TransactionMetadata,
    pub post_accounts: Vec<(Pubkey, AccountSharedData)>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub(crate) tx_result: Result<(), TransactionError>,
    pub(crate) signature: TypedSignature,
    pub(crate) compute_units_consumed: u64,
    pub(crate) inner_instructions: InnerInstructionsList,
    pub(crate) return_data: TransactionReturnData,
    /// Whether the transaction can be included in a block
    pub(crate) included: bool,
    pub(crate) fee: u64,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            post_accounts: Default::default(),
            tx_result: Err(TransactionError::UnsupportedVersion),
            signature: Default::default(),
            compute_units_consumed: Default::default(),
            inner_instructions: Default::default(),
            return_data: Default::default(),
            included: false,
            fee: 0,
        }
    }
}
