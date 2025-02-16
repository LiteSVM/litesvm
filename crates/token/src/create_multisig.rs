use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{
    program_pack::Pack, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

use super::{
    spl_token::{instruction::initialize_multisig2, state::Multisig},
    TOKEN_ID,
};

/// ### Description
/// Builder for the [`initialize_multisig2`] instruction.
pub struct CreateMultisig<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    signers: &'a [&'a Pubkey],
    required_signatures: u8,
    multisig_kp: Option<Keypair>,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> CreateMultisig<'a> {
    /// Creates a new instance of [`initialize_multisig2`] instruction.
    pub fn new(
        svm: &'a mut LiteSVM,
        payer: &'a Keypair,
        signers: &'a [&'a Pubkey],
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
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<Pubkey, FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&TOKEN_ID);
        let multisig_len = Multisig::LEN;
        let multisig_kp = self.multisig_kp.unwrap_or(Keypair::new());
        let multisig_pk = multisig_kp.pubkey();

        let ix1 = system_instruction::create_account(
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
