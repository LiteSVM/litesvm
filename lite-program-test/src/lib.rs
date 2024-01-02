use lite_svm::{
    bank::LiteBank, deploy_program, types::TransactionResult, BuiltinFunctionWithContext, Error,
};
use solana_sdk::pubkey;
use solana_sdk::signer::Signer;
use solana_sdk::{
    account::Account, hash::Hash, pubkey::Pubkey, signature::Keypair,
    transaction::VersionedTransaction,
};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

//TODO
pub struct ProgramTest {
    bank: Arc<RwLock<LiteBank>>,
}

impl ProgramTest {
    pub fn new() -> Self {
        let program_test = Self {
            bank: Arc::new(RwLock::new(LiteBank::new())),
        };
        program_test.load_spl_programs();
        program_test
    }

    fn load_spl_programs(&self) {
        let mut bank = self.get_bank_mut();

        bank.store_program(
            pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            include_bytes!("programs/spl_token-3.5.0.so"),
        );
        bank.store_program(
            pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
            include_bytes!("programs/spl_token_2022-0.9.0.so"),
        );
        bank.store_program(
            pubkey!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"),
            include_bytes!("programs/spl_memo-1.0.0.so"),
        );
        bank.store_program(
            pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
            include_bytes!("programs/spl_memo-3.0.0.so"),
        );
        bank.store_program(
            pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
            include_bytes!("programs/spl_associated_token_account-1.1.1.so"),
        );
    }

    pub fn get_bank(&self) -> RwLockReadGuard<'_, LiteBank> {
        self.bank.read().unwrap()
    }

    pub fn get_bank_mut(&self) -> RwLockWriteGuard<'_, LiteBank> {
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
