use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use spl_token_2022::instruction::{set_authority, AuthorityType};

/// ### Description
/// Builder for the [`set_authority`] instruction.
///
/// ### Optional fields
/// - `owner`: `payer` by default.
/// - `token_program_id`: [`spl_token_2022::ID`] by default.
pub struct SetAuthority<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    authority_type: AuthorityType,
    owner: Option<&'a Keypair>,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> SetAuthority<'a> {
    /// Creates a new instance of [`set_authority`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, authority_type: AuthorityType) -> Self {
        SetAuthority {
            svm,
            payer,
            owner: None,
            authority_type,
            token_program_id: None,
        }
    }

    /// Sets the owner of the spl account.
    pub fn owner(mut self, owner: &'a Keypair) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&spl_token_2022::ID);
        let payer_pk = self.payer.pubkey();
        let owner = self.owner.unwrap_or(self.payer);
        let owner_pk = owner.pubkey();

        let ix = set_authority(
            token_program_id,
            &payer_pk,
            Some(&payer_pk),
            self.authority_type,
            &owner_pk,
            &[],
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer_pk),
            &[self.payer, &owner],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
