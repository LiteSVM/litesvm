use {
    super::{get_multisig_signers, spl_token::instruction::revoke, TOKEN_ID},
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    smallvec::{smallvec, SmallVec},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::{signers::Signers, Signer},
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`revoke`] instruction.
pub struct Revoke<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    source: &'a Address,
    signers: SmallVec<[&'a Keypair; 1]>,
    owner: Option<Address>,
    token_program_id: Option<&'a Address>,
}

impl<'a> Revoke<'a> {
    /// Creates a new instance of [`revoke`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, source: &'a Address) -> Self {
        Revoke {
            svm,
            payer,
            source,
            token_program_id: None,
            owner: None,
            signers: smallvec![payer],
        }
    }

    pub fn owner(mut self, owner: &'a Keypair) -> Self {
        self.owner = Some(owner.pubkey());
        self.signers = smallvec![owner];
        self
    }

    pub fn multisig(mut self, multisig: &'a Address, signers: &'a [&'a Keypair]) -> Self {
        self.owner = Some(*multisig);
        self.signers = SmallVec::from(signers);
        self
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Address) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let payer_pk = self.payer.pubkey();

        let authority = self.owner.unwrap_or(payer_pk);
        let signing_keys = self.signers.pubkeys();
        let signer_keys = get_multisig_signers(&authority, &signing_keys);

        let ix = revoke(token_program_id, self.source, &authority, &signer_keys)?;

        let block_hash = self.svm.latest_blockhash();
        let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
        tx.partial_sign(&[self.payer], block_hash);
        tx.partial_sign(self.signers.as_ref(), block_hash);

        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
