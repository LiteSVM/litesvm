use litesvm::{LiteSVM, types::TransactionResult};
use solana_account::Account;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_transaction::Transaction;
use std::collections::HashMap;

pub mod error;
pub mod merkle;

pub use error::SodaSVMError;
pub use merkle::{SodaMerkleTree, MerkleProof, AccountState};

pub struct SodaSVM {
    lite_svm: LiteSVM,
    user_accounts: HashMap<Pubkey, AccountState>,
    latest_merkle_tree: Option<SodaMerkleTree>,
    last_commit_time: i64,
}

impl SodaSVM {
    pub fn new() -> Self {
        let lite_svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false);

        Self {
            lite_svm,
            user_accounts: HashMap::new(),
            latest_merkle_tree: None,
            last_commit_time: 0,
        }
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

    pub fn generate_state_tree(&self) -> SodaMerkleTree {
        let accounts: Vec<AccountState> = self.user_accounts.values().cloned().collect();
        SodaMerkleTree::new(accounts)
    }

    pub fn commit_state_root(&mut self) -> [u8; 32] {
        let tree = self.generate_state_tree();
        let root = tree.root;

        self.latest_merkle_tree = Some(tree);
        self.last_commit_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        root
    }

    pub fn generate_exit_proof(&self, user_pubkey: &Pubkey) -> Option<MerkleProof> {
        let tree = self.latest_merkle_tree.as_ref()?;

        let account_index = tree.leaves
            .iter()
            .position(|account| account.pubkey == *user_pubkey)?;

        tree.generate_proof(account_index)
    }

    pub fn register_user(&mut self, pubkey: Pubkey, balance: u64) {
        let account_state = AccountState::new(
            pubkey,
            balance,
            0,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        );

        self.user_accounts.insert(pubkey, account_state);
    }

    pub fn update_user_balance(&mut self, pubkey: &Pubkey, new_balance: u64) -> Result<(), String> {
        if let Some(account) = self.user_accounts.get_mut(pubkey) {
            account.balance = new_balance;
            account.nonce += 1;
            account.last_update = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            Ok(())
        } else {
            Err("User not found".to_string())
        }
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

    #[test]
    fn test_merkle_tree_integration() {
        let mut svm = SodaSVM::new();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();

        svm.register_user(user1, 1000);
        svm.register_user(user2, 2000);

        let root = svm.commit_state_root();
        assert_ne!(root, [0; 32]);

        let proof1 = svm.generate_exit_proof(&user1).unwrap();
        assert!(proof1.verify());
        assert_eq!(proof1.account_state.balance, 1000);

        let proof2 = svm.generate_exit_proof(&user2).unwrap();
        assert!(proof2.verify());
        assert_eq!(proof2.account_state.balance, 2000);
    }

    #[test]
    fn test_user_balance_update() {
        let mut svm = SodaSVM::new();
        let user = Pubkey::new_unique();

        svm.register_user(user, 1000);
        svm.update_user_balance(&user, 1500).unwrap();

        let root = svm.commit_state_root();
        let proof = svm.generate_exit_proof(&user).unwrap();

        assert!(proof.verify());
        assert_eq!(proof.account_state.balance, 1500);
        assert_eq!(proof.account_state.nonce, 1);
    }
}