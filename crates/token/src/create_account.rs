#[cfg(not(feature = "token-2022"))]
use solana_program_pack::Pack;
#[cfg(feature = "token-2022")]
use spl_token_2022::extension::ExtensionType;
use {
    super::{
        spl_token::{instruction::initialize_account3, state::Account},
        TOKEN_ID,
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`initialize_account3`] instruction.
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
    #[cfg(feature = "token-2022")]
    extensions: Vec<ExtensionType>,
}

impl<'a> CreateAccount<'a> {
    /// Creates a new instance of the [`initialize_account3`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair, mint: &'a Pubkey) -> Self {
        CreateAccount {
            svm,
            payer,
            mint,
            owner: None,
            account_kp: None,
            token_program_id: None,
            #[cfg(feature = "token-2022")]
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

    /// Sends the transaction.
    pub fn send(self) -> Result<Pubkey, FailedTransactionMetadata> {
        #[cfg(feature = "token-2022")]
        let account_len = ExtensionType::try_calculate_account_len::<Account>(&self.extensions)?;
        #[cfg(not(feature = "token-2022"))]
        let account_len = Account::LEN;

        let lamports = self.svm.minimum_balance_for_rent_exemption(account_len);

        let account_kp = self.account_kp.unwrap_or(Keypair::new());
        let account_pk = account_kp.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let payer_pk = self.payer.pubkey();

        let ix1 = create_account(
            &payer_pk,
            &account_pk,
            lamports,
            account_len as u64,
            token_program_id,
        );

        let ix2 = initialize_account3(
            token_program_id,
            &account_pk,
            self.mint,
            self.owner.unwrap_or(&payer_pk),
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix1, ix2],
            Some(&payer_pk),
            &[self.payer, &account_kp],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(account_pk)
    }
}
