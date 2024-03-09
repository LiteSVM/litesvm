use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct AccountsDb {
    inner: HashMap<Pubkey, AccountSharedData>,
}

impl AccountsDb {
    pub(crate) fn get_account(&self, pubkey: &Pubkey) -> AccountSharedData {
        self.inner
            .get(pubkey)
            .map(|acc| acc.to_owned())
            .unwrap_or_default()
    }

    pub(crate) fn add_account(&mut self, pubkey: Pubkey, data: AccountSharedData) {
        self.inner.insert(pubkey, data);
    }

    pub(crate) fn sync_accounts(&mut self, accounts: Vec<(Pubkey, AccountSharedData)>) {
        for (pubkey, data) in accounts {
            self.add_account(pubkey, data);
        }
    }
}
