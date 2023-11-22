use std::{cell::RefCell, collections::HashMap};

use solana_program::pubkey::Pubkey;
use solana_sdk::account::AccountSharedData;

pub struct AccountsDb {
    inner: RefCell<HashMap<Pubkey, AccountSharedData>>,
}
