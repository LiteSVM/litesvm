use {
    super::{
        spl_token::{instruction::initialize_multisig2, state::Multisig},
        TOKEN_ID,
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_address::Address,
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
};

/// ### Description
/// Builder for the [`initialize_multisig2`] instruction.
pub struct CreateMultisig<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    signers: &'a [&'a Address],
    required_signatures: u8,
    multisig_kp: Option<Keypair>,
    token_program_id: Option<&'a Address>,
}

impl<'a> CreateMultisig<'a> {
    /// Creates a new instance of [`initialize_multisig2`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        signers: &'a [&'a Address],
        required_signatures: u8,
    ) -> Self {
        CreateMultisig {
            svm,
            payer,
            signers,
            multisig_kp: None,
            token_program_id: None,
            required_signatures,
        }
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Address) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<Address, FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let multisig_len = Multisig::LEN;
        let multisig_kp = self.multisig_kp.unwrap_or(Keypair::new());
        let multisig_pk = multisig_kp.pubkey();

        let ix1 = create_account(
            &self.payer.pubkey(),
            &multisig_pk,
            self.svm.minimum_balance_for_rent_exemption(multisig_len),
            multisig_len as u64,
            token_program_id,
        );
        let ix2 = initialize_multisig2(
            token_program_id,
            &multisig_pk,
            self.signers,
            self.required_signatures,
        )?;

        let block_hash = self.svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[ix1, ix2],
            Some(&self.payer.pubkey()),
            &[self.payer, &multisig_kp],
            block_hash,
        );
        self.svm.send_transaction(tx)?;

        Ok(multisig_pk)
    }
}
