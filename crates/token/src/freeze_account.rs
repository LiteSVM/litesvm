use {
    super::{get_multisig_signers, spl_token::instruction::freeze_account, TOKEN_ID},
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    smallvec::{smallvec, SmallVec},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::{signers::Signers, Signer},
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`freeze_account`] instruction.
///
/// ### Optional fields
/// - `account`: associated token account of the owner by default.
/// - `owner`: `payer` by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct FreezeAccount<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Address,
    account: Option<&'a Address>,
    token_program_id: Option<&'a Address>,
    signers: SmallVec<[&'a Keypair; 1]>,
    owner: Option<Address>,
}

impl<'a> FreezeAccount<'a> {
    /// Creates a new instance of [`freeze_account`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, mint: &'a Address) -> Self {
        FreezeAccount {
            svm,
            payer,
            account: None,
            mint,
            owner: None,
            signers: smallvec![payer],
            token_program_id: None,
        }
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

        let account = if let Some(account) = self.account {
            *account
        } else {
            spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
                &authority,
                self.mint,
                token_program_id,
            )
        };

        let ix = freeze_account(
            token_program_id,
            &account,
            self.mint,
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
