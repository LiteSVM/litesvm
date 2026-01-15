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
#[cfg(not(feature = "token-2022"))]
mod create_native_mint;
#[cfg(feature = "token-2022")]
mod create_native_mint_2022;
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
use create_native_mint_2022 as create_native_mint;
#[cfg(feature = "token-2022")]
pub use spl_token_2022_interface as spl_token;
#[cfg(not(feature = "token-2022"))]
pub use spl_token_interface as spl_token;
pub use {
    approve::*, approve_checked::*, burn::*, burn_checked::*, close_account::*, create_account::*,
    create_ata::*, create_ata_idempotent::*, create_mint::*, create_multisig::*,
    create_native_mint::*, freeze_account::*, mint_to::*, mint_to_checked::*, revoke::*,
    set_authority::*, sync_native::*, thaw_account::*, transfer::*, transfer_checked::*,
};
use {
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_address::Address,
    solana_program_pack::{IsInitialized, Pack},
    solana_transaction_error::TransactionError,
};

pub const TOKEN_ID: Address = spl_token::ID;

pub fn get_spl_account<T: Pack + IsInitialized>(
    svm: &LiteSVM,
    account: &Address,
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

fn get_multisig_signers<'a>(
    authority: &Address,
    signing_pubkeys: &'a [Address],
) -> Vec<&'a Address> {
    if signing_pubkeys == [*authority] {
        vec![]
    } else {
        signing_pubkeys.iter().collect::<Vec<_>>()
    }
}
