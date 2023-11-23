use std::{cell::RefCell, collections::HashMap};

use solana_program::pubkey::Pubkey;
use solana_sdk::{account::AccountSharedData, transaction_context::TransactionContext};

use crate::Error;

#[derive(Default)]
pub struct AccountsDb {
    inner: RefCell<HashMap<Pubkey, AccountSharedData>>,
}

impl AccountsDb {
    pub fn get_account(&self, pubkey: &Pubkey) -> AccountSharedData {
        self.inner
            .borrow()
            .get(pubkey)
            .map(|acc| acc.to_owned())
            .unwrap_or_default()
    }

    pub fn sync_accounts(&self, context: &TransactionContext) -> Result<(), Error> {
        for index in 0..context.get_number_of_accounts() {
            let data = context.get_account_at_index(index)?;
            let pubkey = context.get_key_of_account_at_index(index)?;

            self.inner
                .borrow_mut()
                .insert(*pubkey, data.borrow().to_owned());
        }
        Ok(())
    }

    pub fn add_account(&self, pubkey: Pubkey, data: AccountSharedData) {
        self.inner.borrow_mut().insert(pubkey, data);
    }
}
