use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use smallvec::{smallvec, SmallVec};
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, signers::Signers, transaction::Transaction,
};

use super::{
    get_multisig_signers,
    spl_token::instruction::{set_authority, AuthorityType},
    TOKEN_ID,
};

/// ### Description
/// Builder for the [`set_authority`] instruction.
///
/// ### Optional fields
/// - `owner`: `payer` by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct SetAuthority<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    authority_type: AuthorityType,
    account: &'a Pubkey,
    new_authority: Option<&'a Pubkey>,
    signers: SmallVec<[&'a Keypair; 1]>,
    owner: Option<Pubkey>,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> SetAuthority<'a> {
    /// Creates a new instance of [`set_authority`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        account: &'a Pubkey,
        authority_type: AuthorityType,
    ) -> Self {
        SetAuthority {
            svm,
            payer,
            owner: None,
            authority_type,
            account,
            new_authority: None,
            token_program_id: None,
            signers: smallvec![payer],
        }
    }

    /// Sets the owner of the account with single owner.
    pub fn owner(mut self, owner: &'a Keypair) -> Self {
        self.owner = Some(owner.pubkey());
        self.signers = smallvec![owner];
        self
    }

    /// Sets the owner of the account with multisig owner.
    pub fn multisig(mut self, multisig: &'a Pubkey, signers: &'a [&'a Keypair]) -> Self {
        self.owner = Some(*multisig);
        self.signers = SmallVec::from(signers);
        self
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sets the new authority.
    pub fn new_authority(mut self, new_authority: &'a Pubkey) -> Self {
        self.new_authority = Some(new_authority);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let payer_pk = self.payer.pubkey();

        let authority = self.owner.unwrap_or(payer_pk);
        let signing_keys = self.signers.pubkeys();
        let signer_keys = get_multisig_signers(&authority, &signing_keys);

        let ix = set_authority(
            token_program_id,
            self.account,
            self.new_authority,
            self.authority_type,
            &authority,
            &signer_keys,
        )?;

        let block_hash = self.svm.latest_blockhash();
        let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
        tx.partial_sign(&[self.payer], block_hash);
        tx.partial_sign(self.signers.as_ref(), block_hash);

        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
