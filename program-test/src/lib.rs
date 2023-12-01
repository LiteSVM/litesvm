use light_sol_bankrun::bank::LightBank;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

//TODO
pub struct ProgramTest {
    bank: Arc<RwLock<LightBank>>,
}

impl ProgramTest {
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

    // pub fn deploy_program(
    //     &self,
    //     bank: &mut LightBank,
    //     payer_keypair: &Keypair,
    //     program_bytes: &[u8],
    // ) {
    // }
}
