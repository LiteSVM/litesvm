use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use spl_token_2022::instruction::transfer_checked;

use super::get_mint;

/// ### Description
/// Builder for the [`transfer_checked`] instruction.
///
/// ### Optional fields
/// - `source`: associated token account of the `payer` by default.
/// - `authority`: `payer` by default.
/// - `decimals`: `mint` decimals by default.
/// - `token_program_id`: [`spl_token_2022::ID`] by default.
pub struct Transfer<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Pubkey,
    source: Option<&'a Pubkey>,
    destination: &'a Pubkey,
    authority: Option<&'a Keypair>,
    token_program_id: Option<&'a Pubkey>,
    amount: u64,
    decimals: Option<u8>,
}

impl<'a> Transfer<'a> {
    /// Creates a new instance of [`transfer_checked`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Pubkey,
        destination: &'a Pubkey,
        amount: u64,
    ) -> Self {
        Transfer {
            svm,
            payer,
            mint,
            source: None,
            destination,
            authority: None,
            token_program_id: None,
            amount,
            decimals: None,
        }
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sets the decimals of the transfer.
    pub fn decimals(mut self, value: u8) -> Self {
        self.decimals = Some(value);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&spl_token_2022::ID);
        let authority = self.authority.unwrap_or(self.payer);
        let authority_pk = authority.pubkey();
        let payer_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            &payer_pk,
            self.mint,
            token_program_id,
        );
        let source_pk = self.source.unwrap_or(&payer_ata);

        let mint = get_mint(self.svm, self.mint)?;
        let ix = transfer_checked(
            token_program_id,
            source_pk,
            self.mint,
            self.destination,
            &authority_pk,
            &[],
            self.amount,
            self.decimals.unwrap_or(mint.decimals),
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer_pk),
            &[self.payer, authority],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
