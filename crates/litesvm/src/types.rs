use {
    crate::format_logs::format_logs,
    solana_account::AccountSharedData,
    solana_address::Address,
    solana_instruction_error::InstructionError,
    solana_message::inner_instruction::InnerInstructionsList,
    solana_program_error::ProgramError,
    solana_signature::Signature,
    solana_transaction_context::transaction::TransactionReturnData,
    solana_transaction_error::{TransactionError, TransactionResult as Result},
};

/// Native and SPL token balances before and after a transaction.
///
/// Native balances follow the message's account-key order, including loaded addresses.
/// Token balances contain raw amounts for valid token accounts and mints.
#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransactionBalances {
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub pre_token_balances: Vec<TokenBalance>,
    pub post_token_balances: Vec<TokenBalance>,
}

/// A raw SPL Token or Token-2022 balance at an index in the message's account keys.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenBalance {
    pub account_index: u8,
    pub mint: Address,
    pub owner: Address,
    pub amount: u64,
    pub decimals: u8,
    pub program_id: Address,
}

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransactionMetadata {
    #[cfg_attr(feature = "serde", serde(with = "crate::utils::serde_with_str"))]
    pub signature: Signature,
    pub logs: Vec<String>,
    pub inner_instructions: InnerInstructionsList,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
    pub fee: u64,
    /// The size of the accounts the transaction loaded, as SIMD-0186 counts it
    /// against the transaction's loaded accounts data size limit.
    #[cfg_attr(feature = "serde", serde(default))]
    pub loaded_accounts_data_size: u32,
    /// Recorded balances, or `None` when recording is disabled or execution was rejected.
    #[cfg_attr(feature = "serde", serde(default))]
    pub balances: Option<TransactionBalances>,
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
    pub post_accounts: Vec<(Address, AccountSharedData)>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FailedTransactionMetadata {
    pub err: TransactionError,
    pub meta: TransactionMetadata,
    /// Whether the fee was charged, or would be charged when simulating.
    /// Simulation does not debit the live fee payer.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fee_charged: bool,
}

impl From<ProgramError> for FailedTransactionMetadata {
    fn from(value: ProgramError) -> Self {
        FailedTransactionMetadata {
            err: TransactionError::InstructionError(0, InstructionError::from(u64::from(value))),
            meta: Default::default(),
            fee_charged: false,
        }
    }
}

pub type TransactionResult = std::result::Result<TransactionMetadata, FailedTransactionMetadata>;

pub(crate) struct ExecutionResult {
    pub(crate) post_accounts: Vec<(Address, AccountSharedData)>,
    pub(crate) tx_result: Result<()>,
    pub(crate) signature: Signature,
    pub(crate) compute_units_consumed: u64,
    pub(crate) inner_instructions: InnerInstructionsList,
    pub(crate) return_data: TransactionReturnData,
    /// Whether the transaction can be included in a block
    pub(crate) included: bool,
    pub(crate) fee: u64,
    pub(crate) fee_charged: bool,
    pub(crate) balances: Option<TransactionBalances>,
    pub(crate) loaded_accounts_data_size: u32,
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
            fee_charged: false,
            balances: None,
            loaded_accounts_data_size: 0,
        }
    }
}

impl From<TransactionError> for ExecutionResult {
    fn from(err: TransactionError) -> Self {
        Self {
            tx_result: Err(err),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_error_keeps_builtin_variant() {
        assert_eq!(
            FailedTransactionMetadata::from(ProgramError::InvalidAccountData).err,
            TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
        );
    }
}
