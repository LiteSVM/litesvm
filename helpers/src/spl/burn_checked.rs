use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use smallvec::{smallvec, SmallVec};
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, signers::Signers, transaction::Transaction,
};

use super::{
    get_multisig_signers, get_spl_account, spl_token::instruction::burn_checked,
    spl_token::state::Mint, TOKEN_ID,
};

/// ### Description
/// Builder for the [`burn_checked`] instruction.
///
/// ### Optional fields
/// - `authority`: `payer` by default.
/// - `decimals`: `mint` decimals by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct BurnChecked<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Pubkey,
    account: &'a Pubkey,
    token_program_id: Option<&'a Pubkey>,
    amount: u64,
    decimals: Option<u8>,
    signers: SmallVec<[&'a Keypair; 1]>,
    owner: Option<Pubkey>,
}

impl<'a> BurnChecked<'a> {
    /// Creates a new instance of [`burn_checked`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        mint: &'a Pubkey,
        account: &'a Pubkey,
        amount: u64,
    ) -> Self {
        BurnChecked {
            svm,
            payer,
            mint,
            account,
            token_program_id: None,
            amount,
            decimals: None,
            owner: None,
            signers: smallvec![payer],
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

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let payer_pk = self.payer.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);

        let authority = self.owner.unwrap_or(payer_pk);
        let signing_keys = self.signers.pubkeys();
        let signer_keys = get_multisig_signers(&authority, &signing_keys);

        let mint: Mint = get_spl_account(self.svm, self.mint)?;
        let ix = burn_checked(
            token_program_id,
            self.account,
            self.mint,
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
