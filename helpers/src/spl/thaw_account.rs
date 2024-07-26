use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::{spl_token::instruction::thaw_account, TOKEN_ID};

/// ### Description
/// Builder for the [`transfer`] instruction.
///
/// ### Optional fields
/// - `source`: associated token account of the `payer` by default.
/// - `authority`: `payer` by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct ThawAccount<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    account: &'a Pubkey,
    mint: &'a Pubkey,
    owner: Option<&'a Keypair>,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> ThawAccount<'a> {
    /// Creates a new instance of [`transfer`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Pubkey,
        account: &'a Pubkey,
    ) -> Self {
        ThawAccount {
            svm,
            payer,
            mint,
            account,
            owner: None,
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
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let owner = self.owner.unwrap_or(self.payer);
        let owner_pk = owner.pubkey();

        let ix = thaw_account(token_program_id, self.account, self.mint, &owner_pk, &[])?;

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
