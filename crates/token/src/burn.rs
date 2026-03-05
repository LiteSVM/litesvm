use {
    super::{get_multisig_signers, spl_token::instruction::burn, TOKEN_ID},
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    smallvec::{smallvec, SmallVec},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::{signers::Signers, Signer},
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`burn`] instruction.
///
/// ### Optional fields
/// - `authority`: `payer` by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct Burn<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Address,
    account: &'a Address,
    token_program_id: Option<&'a Address>,
    amount: u64,
    signers: SmallVec<[&'a Keypair; 1]>,
    owner: Option<Address>,
}

impl<'a> Burn<'a> {
    /// Creates a new instance of [`burn`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Address,
        account: &'a Address,
        amount: u64,
    ) -> Self {
        Burn {
            svm,
            payer,
            mint,
            account,
            token_program_id: None,
            amount,
            owner: None,
            signers: smallvec![payer],
        }
    }

    /// Sets the token program id of the burn.
    pub fn token_program_id(mut self, program_id: &'a Address) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sets the owner of the account with single owner.
    pub fn owner(mut self, owner: &'a Keypair) -> Self {
        self.owner = Some(owner.pubkey());
        self.signers = smallvec![owner];
        self
    }

    /// Sets the owner of the account with multisig owner.
    pub fn multisig(mut self, multisig: &'a Address, signers: &'a [&'a Keypair]) -> Self {
        self.owner = Some(*multisig);
        self.signers = SmallVec::from(signers);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);

        let authority = self.owner.unwrap_or(payer_pk);
        let signing_keys = self.signers.pubkeys();
        let signer_keys = get_multisig_signers(&authority, &signing_keys);

        let ix = burn(
            token_program_id,
            self.account,
            self.mint,
            &authority,
            &signer_keys,
            self.amount,
        )?;

        let block_hash = self.svm.latest_blockhash();
        let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
        tx.partial_sign(&[self.payer], block_hash);
        tx.partial_sign(self.signers.as_ref(), block_hash);

        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
