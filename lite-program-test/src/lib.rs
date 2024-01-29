use lite_svm::{
    bank::LiteSVM, deploy_program, types::TransactionResult, BuiltinFunctionWithContext, Error,
};
use solana_sdk::signer::Signer;
use solana_sdk::{
    account::Account, hash::Hash, pubkey::Pubkey, signature::Keypair,
    transaction::VersionedTransaction,
};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

//TODO
pub struct ProgramTest {
    bank: Arc<RwLock<LiteSVM>>,
}

impl ProgramTest {
    pub fn new() -> Self {
        let program_test = Self {
            bank: Arc::new(RwLock::new(LiteSVM::new())),
        };
        program_test
    }

    pub fn get_bank(&self) -> RwLockReadGuard<'_, LiteSVM> {
        self.bank.read().unwrap()
    }

    pub fn get_bank_mut(&self) -> RwLockWriteGuard<'_, LiteSVM> {
        self.bank.write().unwrap()
    }

    pub fn request_airdrop(&self, pubkey: &Pubkey, lamports: u64) {
        self.get_bank_mut().airdrop(pubkey, lamports).unwrap()
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Account {
        self.get_bank().get_account(pubkey)
    }

    pub fn store_program(&self, program_id: Pubkey, program_bytes: &[u8]) {
        self.get_bank_mut().store_program(program_id, program_bytes)
    }

    pub fn get_minimum_balance_for_rent_exemption(&self, len: usize) -> u64 {
        self.get_bank().minimum_balance_for_rent_exemption(len)
    }

    pub fn send_transaction(
        &self,
        tx: impl Into<VersionedTransaction>,
    ) -> Result<TransactionResult, Error> {
        self.get_bank_mut().send_transaction(tx.into())
    }

    pub fn get_latest_blockhash(&self) -> Hash {
        self.get_bank().latest_blockhash()
    }

    pub fn deploy_program(&self, owner: &Keypair, program_bytes: &[u8]) -> Pubkey {
        let bank = &mut self.get_bank_mut();
        deploy_program(bank, owner, program_bytes).unwrap()
    }

    pub fn deploy_builtin(&self, entrypoint: BuiltinFunctionWithContext) -> Pubkey {
        let program_kp = Keypair::new();
        let program_id = program_kp.pubkey();
        self.get_bank_mut().add_builtin(program_id, entrypoint);
        program_id
    }
}
