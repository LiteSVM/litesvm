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
}

impl TransactionMetadata {
    pub fn pretty_logs(&self) -> String {
        format_logs(&self.logs)
    }

    pub fn cpi_tree(&self) -> Vec<crate::cpi_tree::CpiFrame> {
        crate::cpi_tree::cpi_tree(&self.logs)
    }

    pub fn pretty_cpi_tree(&self) -> String {
        use crate::cpi_tree::{
            format_cpi_tree, transaction_compute_budget, transaction_total_cu, with_commas,
        };
        let frames = self.cpi_tree();
        // Same header agave's `solana logs --tree` builds: transaction-total
        // BPF CU and the budget, or an explicit no-data note. Never "0 CU":
        // native programs don't emit `consumed` lines, and reporting that
        // absence as zero would misstate the cost.
        let header = match (
            transaction_total_cu(&frames),
            transaction_compute_budget(&frames),
        ) {
            (Some(total), Some(budget)) => format!(
                "CPI Tree ({} BPF CU / {} budget):",
                with_commas(total),
                with_commas(budget)
            ),
            _ => "CPI Tree (no compute units in logs):".to_string(),
        };
        format_cpi_tree(&header, &frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_with_logs(logs: Vec<String>) -> TransactionMetadata {
        TransactionMetadata {
            logs,
            ..Default::default()
        }
    }

    #[test]
    fn pretty_cpi_tree_header_shows_total_and_budget() {
        let meta = meta_with_logs(vec![
            "Program GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2 invoke [1]".to_string(),
            "Program GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2 consumed 4817 of 1000000 \
             compute units"
                .to_string(),
            "Program GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2 success".to_string(),
        ]);
        let out = meta.pretty_cpi_tree();
        assert!(
            out.starts_with("CPI Tree (4,817 BPF CU / 1,000,000 budget):"),
            "unexpected header: {out}"
        );
    }

    #[test]
    fn pretty_cpi_tree_header_notes_missing_cu() {
        // System-program-only transaction: native programs never emit
        // `consumed` lines, so there's no CU data to total.
        let meta = meta_with_logs(vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ]);
        let out = meta.pretty_cpi_tree();
        assert!(
            out.starts_with("CPI Tree (no compute units in logs):"),
            "unexpected header: {out}"
        );
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
    pub(crate) post_accounts: Vec<(Address, AccountSharedData)>,
    pub(crate) tx_result: Result<()>,
    pub(crate) signature: Signature,
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
