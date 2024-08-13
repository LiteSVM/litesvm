mod approve;
mod approve_checked;
mod burn;
mod burn_checked;
mod close_account;
mod create_account;
mod create_ata;
mod create_ata_idempotent;
mod create_mint;
mod create_multisig;
#[cfg(feature = "token-2022")]
mod create_native_mint;
mod freeze_account;
mod mint_to;
mod mint_to_checked;
mod revoke;
mod set_authority;
mod sync_native;
mod thaw_account;
mod transfer;
mod transfer_checked;

pub use approve::*;
pub use approve_checked::*;
pub use burn::*;
pub use burn_checked::*;
pub use close_account::*;
pub use create_account::*;
pub use create_ata::*;
pub use create_ata_idempotent::*;
pub use create_mint::*;
pub use create_multisig::*;
#[cfg(feature = "token-2022")]
pub use create_native_mint::*;
pub use freeze_account::*;
pub use mint_to::*;
pub use mint_to_checked::*;
pub use revoke::*;
pub use set_authority::*;
pub use sync_native::*;
pub use thaw_account::*;
pub use transfer::*;
pub use transfer_checked::*;

#[cfg(feature = "token-2022")]
pub use spl_token_2022 as spl_token;

#[cfg(all(feature = "token", not(feature = "token-2022")))]
pub use spl_token;

use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    transaction::TransactionError,
};

pub const TOKEN_ID: Pubkey = spl_token::ID;

pub fn get_spl_account<T: Pack + IsInitialized>(
    svm: &LiteSVM,
    account: &Pubkey,
) -> Result<T, FailedTransactionMetadata> {
    let account = T::unpack(
        &svm.get_account(account)
            .ok_or(FailedTransactionMetadata {
                err: TransactionError::AccountNotFound,
                meta: Default::default(),
            })?
            .data,
    )?;

    Ok(account)
}

fn get_multisig_signers<'a>(authority: &Pubkey, signing_pubkeys: &'a [Pubkey]) -> Vec<&'a Pubkey> {
    if signing_pubkeys == [*authority] {
        vec![]
    } else {
        signing_pubkeys.iter().collect::<Vec<_>>()
    }
}
