use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};
use spl_token_2022::{extension::ExtensionType, state::Account};

/// ### Description
/// Builder for the spl account creation transaction.
///
/// ### Optional fields
/// - `owner`: `payer` by default.
/// - `account_kp`: [`Keypair::new()`] by default.
pub struct CreateAccount<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    mint: &'a Pubkey,
    owner: Option<&'a Pubkey>,
    account_kp: Option<Keypair>,
    token_program_id: Option<&'a Pubkey>,
    extensions: Vec<ExtensionType>,
}

impl<'a> CreateAccount<'a> {
    /// Creates a new instance of the spl account creation transaction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, mint: &'a Pubkey) -> Self {
        CreateAccount {
            svm,
            payer,
            mint,
            owner: None,
            account_kp: None,
            token_program_id: None,
            extensions: vec![],
        }
    }

    /// Sets the owner of the spl account.
    pub fn owner(mut self, owner: &'a Pubkey) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Sets the [`Keypair`] of the spl account.
    pub fn account_kp(mut self, account_kp: Keypair) -> Self {
        self.account_kp = Some(account_kp);
        self
    }

    /// Sets the token program id of the spl account.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    // pub fn extension(mut self, extension: ExtensionType) -> Self {
    //     self
    // }

    /// Sends the transaction.
    pub fn send(self) -> Result<Pubkey, FailedTransactionMetadata> {
        let account_len = ExtensionType::try_calculate_account_len::<Account>(&self.extensions)?;
        let lamports = self.svm.minimum_balance_for_rent_exemption(account_len);

        let account_kp = self.account_kp.unwrap_or(Keypair::new());
        let account_pk = account_kp.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&spl_token_2022::ID);
        let payer_pk = self.payer.pubkey();

        let ix1 = system_instruction::create_account(
            &self.payer.pubkey(),
            &account_pk,
            lamports,
            account_len as u64,
            token_program_id,
        );

        let ix2 = spl_token_2022::instruction::initialize_account3(
            token_program_id,
            &account_pk,
            self.mint,
            self.owner.unwrap_or(&payer_pk),
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix1, ix2],
            Some(&payer_pk),
            &[&self.payer, &account_kp],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(account_pk)
    }
}
