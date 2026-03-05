#[cfg(not(feature = "token-2022"))]
use solana_program_pack::Pack;
#[cfg(feature = "token-2022")]
use spl_token_2022_interface::extension::ExtensionType;
use {
    super::{
        spl_token::{instruction::initialize_mint2, state::Mint},
        TOKEN_ID,
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`initialize_mint2`] instruction.
///
/// ### Optional fields
/// - `authority`: `payer` by default.
/// - `freeze_authority`: None by default.
/// - `decimals`: 8 by default.
/// - `token_program_id`: [`TOKEN_ID`] by default.
pub struct CreateMint<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    authority: Option<&'a Address>,
    freeze_authority: Option<&'a Address>,
    decimals: Option<u8>,
    token_program_id: Option<&'a Address>,
}

impl<'a> CreateMint<'a> {
    /// Creates a new instance of the [`initialize_mint2`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair) -> Self {
        CreateMint {
            svm,
            payer,
            authority: None,
            freeze_authority: None,
            decimals: None,
            token_program_id: None,
        }
    }

    /// Sets the authority of the mint.
    pub fn authority(mut self, authority: &'a Address) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Sets the freeze authority of the mint.
    pub fn freeze_authority(mut self, freeze_authority: &'a Address) -> Self {
        self.freeze_authority = Some(freeze_authority);
        self
    }

    /// Sets the decimals of the mint.
    pub fn decimals(mut self, value: u8) -> Self {
        self.decimals = Some(value);
        self
    }

    /// Sets the token program id of the mint account.
    pub fn token_program_id(mut self, program_id: &'a Address) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<Address, FailedTransactionMetadata> {
        #[cfg(feature = "token-2022")]
        let mint_size = ExtensionType::try_calculate_account_len::<Mint>(&[])?;
        #[cfg(not(feature = "token-2022"))]
        let mint_size = Mint::LEN;
        let mint_kp = Keypair::new();
        let mint_pk = mint_kp.pubkey();
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let payer_pk = self.payer.pubkey();

        let ix1 = create_account(
            &payer_pk,
            &mint_pk,
            self.svm.minimum_balance_for_rent_exemption(mint_size),
            mint_size as u64,
            token_program_id,
        );
        let ix2 = initialize_mint2(
            token_program_id,
            &mint_pk,
            self.authority.unwrap_or(&payer_pk),
            self.freeze_authority,
            self.decimals.unwrap_or(8),
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix1, ix2],
            Some(&payer_pk),
            &[self.payer, &mint_kp],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(mint_pk)
    }
}
