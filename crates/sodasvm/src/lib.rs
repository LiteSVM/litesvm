use litesvm::{LiteSVM, types::TransactionResult};
use solana_account::Account;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_transaction::Transaction;

pub mod error;

pub use error::SodaSVMError;

pub struct SodaSVM {
    lite_svm: LiteSVM,
}

impl SodaSVM {
    pub fn new() -> Self {
        let lite_svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false);

        Self { lite_svm }
    }

    pub fn send_transaction_free(&mut self, tx: Transaction) -> TransactionResult {
        self.lite_svm.send_transaction(tx)
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        self.lite_svm.get_account(pubkey)
    }

    pub fn get_balance(&self, pubkey: &Pubkey) -> Option<u64> {
        self.lite_svm.get_balance(pubkey)
    }

    pub fn airdrop(&mut self, pubkey: &Pubkey, lamports: u64) -> TransactionResult {
        self.lite_svm.airdrop(pubkey, lamports)
    }

    pub fn latest_blockhash(&self) -> Hash {
        self.lite_svm.latest_blockhash()
    }

    pub fn add_program(
        &mut self,
        program_id: impl Into<Pubkey>,
        program_bytes: &[u8],
    ) -> Result<(), litesvm::error::LiteSVMError> {
        self.lite_svm.add_program(program_id, program_bytes)
    }

    pub fn set_account(
        &mut self,
        pubkey: Pubkey,
        account: Account,
    ) -> Result<(), litesvm::error::LiteSVMError> {
        self.lite_svm.set_account(pubkey, account)
    }
}

impl Default for SodaSVM {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_keypair::Keypair;
    use solana_signer::Signer;
    use solana_system_interface::instruction::transfer;
    use solana_message::Message;

    #[test]
    fn test_fee_free_transfer() {
        let mut svm = SodaSVM::new();
        let from_kp = Keypair::new();
        let to_kp = Keypair::new();

        let initial_amount = 1_000_000;
        svm.airdrop(&from_kp.pubkey(), initial_amount).unwrap();

        let transfer_amount = 100_000;
        let instruction = transfer(&from_kp.pubkey(), &to_kp.pubkey(), transfer_amount);
        let message = Message::new(&[instruction], Some(&from_kp.pubkey()));
        let tx = Transaction::new(&[&from_kp], message, svm.latest_blockhash());

        let result = svm.send_transaction_free(tx);
        assert!(result.is_ok());

        let to_balance = svm.get_balance(&to_kp.pubkey()).unwrap();
        assert_eq!(to_balance, transfer_amount);
    }
}