use light_sol_bankrun::bank::LightBank;
use solana_sdk::pubkey;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

//TODO
pub struct ProgramTest {
    bank: Arc<RwLock<LightBank>>,
}

impl ProgramTest {
    pub fn new() -> Self {
        let program_test = Self {
            bank: Arc::new(RwLock::new(LightBank::new())),
        };
        program_test.load_spl_programs();
        program_test
    }

    pub fn load_spl_programs(&self) {
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

    pub fn get_bank(&self) -> RwLockReadGuard<'_, LightBank> {
        self.bank.read().unwrap()
    }

    pub fn get_bank_mut(&self) -> RwLockWriteGuard<'_, LightBank> {
        self.bank.write().unwrap()
    }

    pub fn request_airdrop(&self, pubkey: &Pubkey, lamports: u64) {
        self.get_bank_mut().airdrop(pubkey, lamports).unwrap()
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Account {
        self.get_bank_mut().get_account(pubkey)
    }

    pub fn store_program(&self, program_id: Pubkey, program_bytes: &[u8]) {
        self.get_bank_mut().store_program(program_id, program_bytes)
    }
    // pub fn deploy_program(
    //     &self,
    //     bank: &mut LightBank,
    //     payer_keypair: &Keypair,
    //     program_bytes: &[u8],
    // ) {
    // }
}
