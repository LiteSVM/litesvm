use {
    super::{get_multisig_signers, spl_token::instruction::mint_to, TOKEN_ID},
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    smallvec::{smallvec, SmallVec},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::{signers::Signers, Signer},
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`mint_to`] instruction.
///
/// ### Optional fields
/// - `owner`: payer by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct MintTo<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Address,
    destination: &'a Address,
    token_program_id: Option<&'a Address>,
    owner: Option<Address>,
    signers: SmallVec<[&'a Keypair; 1]>,
    amount: u64,
}

impl<'a> MintTo<'a> {
    /// Creates a new instance of [`mint_to`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Address,
        destination: &'a Address,
        amount: u64,
    ) -> Self {
        MintTo {
            svm,
            payer,
            mint,
            destination,
            token_program_id: None,
            signers: smallvec![payer],
            owner: None,
            amount,
        }
    }

    /// Sets the token program id of the mint account.
    pub fn token_program_id(mut self, program_id: &'a Address) -> Self {
        self.token_program_id = Some(program_id);
        self
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

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);

        let authority = self.owner.unwrap_or(payer_pk);
        let signing_keys = self.signers.pubkeys();
        let signer_keys = get_multisig_signers(&authority, &signing_keys);

        let ix = mint_to(
            token_program_id,
            self.mint,
            self.destination,
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
