//! `ScopeExt`: decode straight off a `TransactionMetadata`.

use {
    crate::{tree, DecodedTransaction, IdlRegistry},
    litesvm::{
        types::{FailedTransactionMetadata, TransactionMetadata},
        LiteSVM,
    },
    litesvm_cpi_tree::cpi_tree,
    solana_address::Address,
    solana_message::VersionedMessage,
    solana_transaction_error::TransactionError,
};

/// Decoding on a transaction's metadata. Bring it into scope
/// (`use litesvm_scope::ScopeExt;`) to call these on a
/// [`TransactionMetadata`] or [`FailedTransactionMetadata`].
pub trait ScopeExt {
    /// Decode every instruction and CPI of the transaction that produced this
    /// metadata, resolving lookup tables from `svm`, and attach outcomes,
    /// compute units, named errors and events from the logs.
    fn decode(
        &self,
        svm: &LiteSVM,
        message: &VersionedMessage,
        registry: &IdlRegistry,
    ) -> DecodedTransaction {
        let keys = tree::resolve_account_keys(svm, message);
        self.decode_with_keys(message, &keys, registry)
    }

    /// [`ScopeExt::decode`] with the resolved account keys supplied.
    fn decode_with_keys(
        &self,
        message: &VersionedMessage,
        keys: &[Address],
        registry: &IdlRegistry,
    ) -> DecodedTransaction;
}

fn build(
    meta: &TransactionMetadata,
    err: Option<&TransactionError>,
    message: &VersionedMessage,
    keys: &[Address],
    registry: &IdlRegistry,
) -> DecodedTransaction {
    let mut instructions =
        tree::decode_instructions(message, keys, &meta.inner_instructions, registry);
    let frames = cpi_tree(&meta.logs);
    tree::attach_frames(&mut instructions, &frames, registry);
    let error = err.map(|e| {
        crate::errors::describe_transaction_error(registry, e, |i| {
            instructions.get(i).map(|ix| ix.program)
        })
    });
    // A failed transaction whose logs did not pinpoint the frame: mark the
    // runtime's own instruction index so `failing_instruction` still answers.
    if let (Some(TransactionError::InstructionError(index, _)), Some(error)) = (err, &error) {
        if let Some(top) = instructions.get_mut(*index as usize) {
            if top.walk().all(|ix| ix.success != Some(false)) {
                top.success = Some(false);
                top.error = Some(error.clone());
            }
        }
    }
    DecodedTransaction {
        instructions,
        error,
    }
}

impl ScopeExt for TransactionMetadata {
    fn decode_with_keys(
        &self,
        message: &VersionedMessage,
        keys: &[Address],
        registry: &IdlRegistry,
    ) -> DecodedTransaction {
        build(self, None, message, keys, registry)
    }
}

impl ScopeExt for FailedTransactionMetadata {
    fn decode_with_keys(
        &self,
        message: &VersionedMessage,
        keys: &[Address],
        registry: &IdlRegistry,
    ) -> DecodedTransaction {
        build(&self.meta, Some(&self.err), message, keys, registry)
    }
}
