//! `CpiTreeExt`: the convenience layer that hangs the parser and renderer off
//! litesvm's `TransactionMetadata`. The parsing and rendering live in this
//! crate's root; this module just routes a metadata value's `logs` through
//! them. litesvm owns the metadata type, this crate owns the trait, so the
//! dependency points helper -> litesvm like every other litesvm sibling.

use {
    crate::{
        format_cpi_tree, transaction_compute_budget, transaction_total_cu, with_commas, CpiFrame,
    },
    litesvm::types::TransactionMetadata,
};

/// CPI-tree access on a transaction's metadata. Bring it into scope
/// (`use litesvm_cpi_tree::CpiTreeExt;`) to call these on a
/// [`TransactionMetadata`].
pub trait CpiTreeExt {
    /// Parse this transaction's logs into a tree of CPI frames.
    fn cpi_tree(&self) -> Vec<CpiFrame>;

    /// Render the CPI tree as `cargo tree`-style box art under a header
    /// reporting the transaction's BPF CU and budget.
    fn pretty_cpi_tree(&self) -> String;
}

impl CpiTreeExt for TransactionMetadata {
    fn cpi_tree(&self) -> Vec<CpiFrame> {
        crate::cpi_tree(&self.logs)
    }

    fn pretty_cpi_tree(&self) -> String {
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
    use {super::CpiTreeExt, litesvm::types::TransactionMetadata};

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
