use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::{get_mint, spl_token::instruction::burn_checked, TOKEN_ID};

/// ### Description
/// Builder for the [`burn_checked`] instruction.
///
/// ### Optional fields
/// - `authority`: `payer` by default.
/// - `decimals`: `mint` decimals by default.
/// - `token_program_id`: [`spl_token_2022::ID`] by default.
pub struct Burn<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Pubkey,
    account: &'a Pubkey,
    authority: Option<&'a Keypair>,
    token_program_id: Option<&'a Pubkey>,
    amount: u64,
    decimals: Option<u8>,
}

impl<'a> Burn<'a> {
    /// Creates a new instance of [`burn_checked`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Pubkey,
        account: &'a Pubkey,
        amount: u64,
    ) -> Self {
        Burn {
            svm,
            payer,
            mint,
            authority: None,
            account,
            token_program_id: None,
            amount,
            decimals: None,
        }
    }

    /// Sets the token program id of the burn.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sets the decimals of the burn.
    pub fn decimals(mut self, value: u8) -> Self {
        self.decimals = Some(value);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let authority = self.authority.unwrap_or(self.payer);
        let authority_pk = authority.pubkey();

        let mint = get_mint(self.svm, self.mint)?;
        let ix = burn_checked(
            token_program_id,
            self.account,
            self.mint,
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
