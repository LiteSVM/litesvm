use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use std::collections::HashMap;

#[derive(Default)]
pub struct AccountsDb {
    inner: HashMap<Pubkey, AccountSharedData>,
}

impl AccountsDb {
    pub fn get_account(&self, pubkey: &Pubkey) -> AccountSharedData {
        self.inner
            .get(pubkey)
            .map(|acc| acc.to_owned())
            .unwrap_or_default()
    }

    pub fn add_account(&mut self, pubkey: Pubkey, data: AccountSharedData) {
        self.inner.insert(pubkey, data);
    }

    pub fn sync_accounts(&mut self, accounts: Vec<(Pubkey, AccountSharedData)>) {
        for (pubkey, data) in accounts {
            if let Some(existing_account) = self.inner.get_mut(&pubkey) {
                *existing_account = data;
            } else {
                self.inner.insert(pubkey, data);
            }
        }
    }
}
