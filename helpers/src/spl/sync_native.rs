use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::{spl_token::instruction::sync_native, TOKEN_ID};

/// ### Description
/// Builder for the [`sync_native`] instruction.
///
/// ### Optional fields
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct SyncNative<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    account: &'a Pubkey,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> SyncNative<'a> {
    /// Creates a new instance of [`sync_native`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, account: &'a Pubkey) -> Self {
        SyncNative {
            svm,
            payer,
            account,
            token_program_id: None,
        }
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);

        let ix = sync_native(token_program_id, self.account)?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[self.payer],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
