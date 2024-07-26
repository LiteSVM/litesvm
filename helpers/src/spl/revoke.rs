use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::{spl_token::instruction::revoke, TOKEN_ID};

/// ### Description
/// Builder for the [`revoke`] instruction.
pub struct Revoke<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    source: &'a Pubkey,
    owner: Option<&'a Keypair>,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> Revoke<'a> {
    /// Creates a new instance of [`revoke`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, source: &'a Pubkey) -> Self {
        Revoke {
            svm,
            payer,
            owner: None,
            source,
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
        let payer_pk = self.payer.pubkey();
        let owner = self.owner.unwrap_or(self.payer);
        let owner_pk = owner.pubkey();

        let ix = revoke(token_program_id, self.source, &owner_pk, &[])?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer_pk),
            &[self.payer, owner],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
