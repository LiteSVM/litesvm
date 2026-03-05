use {
    super::{
        get_multisig_signers, get_spl_account,
        spl_token::{instruction::approve_checked, state::Mint},
        TOKEN_ID,
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    smallvec::{smallvec, SmallVec},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::{signers::Signers, Signer},
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`approve_checked`] instruction.
///
/// ### Optional fields
/// - `source`: associated token account of the `payer` by default.
/// - `owner`: `payer` by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct ApproveChecked<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    delegate: &'a Address,
    mint: &'a Address,
    source: Option<&'a Address>,
    amount: u64,
    signers: SmallVec<[&'a Keypair; 1]>,
    owner: Option<Address>,
    token_program_id: Option<&'a Address>,
    decimals: Option<u8>,
}

impl<'a> ApproveChecked<'a> {
    /// Creates a new instance of [`approve_checked`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        delegate: &'a Address,
        mint: &'a Address,
        amount: u64,
    ) -> Self {
        ApproveChecked {
            svm,
            payer,
            delegate,
            mint,
            source: None,
            token_program_id: None,
            amount,
            owner: None,
            signers: smallvec![payer],
            decimals: None,
        }
    }

    /// Sets the token account source.
    pub fn source(mut self, source: &'a Address) -> Self {
        self.source = Some(source);
        self
    }

    /// Sets the token program id.
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

        let source_pk = if let Some(source) = self.source {
            *source
        } else {
            spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
                &authority,
                self.mint,
                token_program_id,
            )
        };

        let mint: Mint = get_spl_account(self.svm, self.mint)?;
        let ix = approve_checked(
            token_program_id,
            &source_pk,
            self.mint,
            self.delegate,
            &authority,
            &signer_keys,
            self.amount,
            self.decimals.unwrap_or(mint.decimals),
        )?;

        let block_hash = self.svm.latest_blockhash();
        let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
        tx.partial_sign(&[self.payer], block_hash);
        tx.partial_sign(self.signers.as_ref(), block_hash);

        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
