mod burn;
mod close_account;
mod create_account;
mod create_mint;
mod create_multi_sig;
#[cfg(feature = "token-2022")]
mod create_native_mint;
mod mint_to;
mod revoke;
mod set_authority;
mod sync_native;
mod transfer;

pub use burn::*;
pub use close_account::*;
pub use create_account::*;
pub use create_mint::*;
pub use create_multi_sig::*;
#[cfg(feature = "token-2022")]
pub use create_native_mint::*;
pub use mint_to::*;
pub use revoke::*;
pub use set_authority::*;
pub use sync_native::*;
pub use transfer::*;

#[cfg(feature = "token-2022")]
use spl_token_2022 as spl_token;

#[cfg(feature = "token")]
use spl_token;

use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, transaction::TransactionError};
use spl_token::state::{Account, Mint};

const TOKEN_ID: Pubkey = spl_token::ID;

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
