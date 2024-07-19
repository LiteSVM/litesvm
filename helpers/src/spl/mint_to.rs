use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::{spl_token::instruction::mint_to, TOKEN_ID};

/// ### Description
/// Builder for the [`mint_to`] instruction.
///
/// ### Optional fields
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct MintTo<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Pubkey,
    destination: &'a Pubkey,
    token_program_id: Option<&'a Pubkey>,
    amount: u64,
}

impl<'a> MintTo<'a> {
    /// Creates a new instance of [`mint_to`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Pubkey,
        destination: &'a Pubkey,
        amount: u64,
    ) -> Self {
        MintTo {
            svm,
            payer,
            mint,
            destination,
            token_program_id: None,
            amount,
        }
    }

    /// Sets the token program id of the mint account.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);

        let ix = mint_to(
            token_program_id,
            self.mint,
            self.destination,
            &payer_pk,
            &[],
            self.amount,
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&self.payer], block_hash);
        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
