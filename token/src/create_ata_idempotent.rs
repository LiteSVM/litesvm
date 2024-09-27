use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;

use super::TOKEN_ID;

/// ### Description
/// Builder for the [`create_associated_token_account_idempotent`] instruction.
///
/// ### Optional fields
/// - `owner`: `payer` by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct CreateAssociatedTokenAccountIdempotent<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Pubkey,
    token_program_id: Option<&'a Pubkey>,
    owner: Option<Pubkey>,
}

impl<'a> CreateAssociatedTokenAccountIdempotent<'a> {
    /// Creates a new instance of [`create_associated_token_account_idempotent`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, mint: &'a Pubkey) -> Self {
        CreateAssociatedTokenAccountIdempotent {
            svm,
            payer,
            owner: None,
            token_program_id: None,
            mint,
        }
    }

    /// Sets the owner of the account with single owner.
    pub fn owner(mut self, owner: &'a Pubkey) -> Self {
        self.owner = Some(*owner);
        self
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<Pubkey, FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let payer_pk = self.payer.pubkey();

        let authority = self.owner.unwrap_or(payer_pk);

        let ix = create_associated_token_account_idempotent(
            &payer_pk,
            &authority,
            self.mint,
            token_program_id,
        );

        let block_hash = self.svm.latest_blockhash();
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[self.payer], block_hash);

        self.svm.send_transaction(tx)?;

        let ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            &authority,
            self.mint,
            token_program_id,
        );

        Ok(ata)
    }
}
