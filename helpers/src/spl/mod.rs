mod burn;
mod close_account;
mod create_account;
mod create_mint;
mod mint_to;
mod set_authority;
mod transfer;

pub use burn::*;
pub use close_account::*;
pub use create_account::*;
pub use create_mint::*;
pub use mint_to::*;
pub use set_authority::*;
pub use transfer::*;

use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, transaction::TransactionError};
use spl_token_2022::state::{Account, Mint};

pub fn get_mint(svm: &LiteSVM, mint: &Pubkey) -> Result<Mint, FailedTransactionMetadata> {
    let mint = Mint::unpack(
        &svm.get_account(mint)
            .ok_or(FailedTransactionMetadata {
                err: TransactionError::AccountNotFound,
                meta: Default::default(),
            })?
            .data,
    )?;

    Ok(mint)
}

pub fn get_account(svm: &LiteSVM, account: &Pubkey) -> Result<Account, FailedTransactionMetadata> {
    let account = Account::unpack(
        &svm.get_account(account)
            .ok_or(FailedTransactionMetadata {
                err: TransactionError::AccountNotFound,
                meta: Default::default(),
            })?
            .data,
    )?;

    Ok(account)
}
