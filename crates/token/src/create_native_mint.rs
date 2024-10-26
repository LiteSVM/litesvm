use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use spl_token_2022::instruction::create_native_mint;

/// ### Description
/// Builder for the [`create_native_mint`] instruction.
pub struct CreateNativeMint<'a> {
    svm: &'a mut LiteSVM,
    payer: &'a Keypair,
    token_program_id: Option<&'a Pubkey>,
}

impl<'a> CreateNativeMint<'a> {
    /// Creates a new instance of [`create_native_mint`] instruction.
    pub fn new(svm: &'a mut LiteSVM, payer: &'a Keypair) -> Self {
        CreateNativeMint {
            svm,
            payer,
            token_program_id: None,
        }
    }

    /// Sets the token program id for the instruction.
    pub fn token_program_id(mut self, program_id: &'a Pubkey) -> Self {
        self.token_program_id = Some(program_id);
        self
    }

    /// Sends the transaction.
    pub fn send(self) -> Result<(), FailedTransactionMetadata> {
        let token_program_id = self.token_program_id.unwrap_or(&spl_token_2022::ID);
        let payer_pk = self.payer.pubkey();

        let ix = create_native_mint(token_program_id, &payer_pk)?;

        let block_hash = self.svm.latest_blockhash();
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[self.payer], block_hash);
        self.svm.send_transaction(tx)?;

        Ok(())
    }
}
