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

#[cfg(feature = "token-2022")]
pub use create_native_mint::*;
#[cfg(not(feature = "token-2022"))]
pub use spl_token;
#[cfg(feature = "token-2022")]
pub use spl_token_2022 as spl_token;
pub use {
    approve::*, approve_checked::*, burn::*, burn_checked::*, close_account::*, create_account::*,
    create_ata::*, create_ata_idempotent::*, create_mint::*, create_multisig::*, freeze_account::*,
    mint_to::*, mint_to_checked::*, revoke::*, set_authority::*, sync_native::*, thaw_account::*,
    transfer::*, transfer_checked::*,
};
use {
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_program_pack::{IsInitialized, Pack},
    solana_pubkey::Pubkey,
    solana_transaction_error::TransactionError,
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
            .data[..T::LEN],
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
