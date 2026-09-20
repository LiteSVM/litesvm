//! Internal constructors for the raw [`Account`] values the builder fixtures
//! and the send backfill install. Kept apart from the public [`fixture`] module:
//! this is the account-bytes factory, not the builder API.
//!
//! [`fixture`]: crate::fixture

use {
    crate::{system_program, Account, Pubkey, SPL_ASSOCIATED_TOKEN_PROGRAM_ID},
    solana_rent::Rent,
    spl_token::{
        solana_program::program_pack::Pack,
        state::{Account as TokenAccount, AccountState, Mint},
    },
};

/// A system-owned account holding `lamports` and no data.
pub(crate) fn system_account(address: Pubkey, lamports: u64) -> Account {
    Account::new(address, system_program::ID, lamports, Vec::new())
}

/// An empty system-owned account, suitable for an init target.
pub(crate) fn empty_account(address: Pubkey) -> Account {
    system_account(address, 0)
}

/// A rent-exempt program-owned account containing `data`.
pub(crate) fn program_account(address: Pubkey, owner: Pubkey, data: Vec<u8>) -> Account {
    Account::new(
        address,
        owner,
        Rent::default().minimum_balance(data.len()),
        data,
    )
}

/// An initialized SPL mint owned by `token_program`, packed at its canonical size.
pub(crate) fn token_program_mint_account(
    address: Pubkey,
    mint_authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
    supply: u64,
    decimals: u8,
    token_program: Pubkey,
) -> Account {
    let mint = Mint {
        mint_authority: mint_authority.into(),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: freeze_authority.into(),
    };
    let mut data = vec![0; Mint::LEN];
    Mint::pack(mint, &mut data).expect("mint fixture buffer has the canonical size");
    program_account(address, token_program, data)
}

/// An initialized SPL token account owned by `token_program`, packed at its
/// canonical size.
pub(crate) fn token_program_account(
    address: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    token_program: Pubkey,
) -> Account {
    let token_account = TokenAccount {
        mint,
        owner,
        amount,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    };
    let mut data = vec![0; TokenAccount::LEN];
    TokenAccount::pack(token_account, &mut data)
        .expect("token-account fixture buffer has the canonical size");
    program_account(address, token_program, data)
}

/// An initialized SPL token account at the associated-token address for
/// `wallet` and `mint`, owned by `token_program`.
pub(crate) fn associated_token_account_with_program(
    wallet: Pubkey,
    mint: Pubkey,
    amount: u64,
    token_program: Pubkey,
) -> Account {
    let address = Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0;
    token_program_account(address, mint, wallet, amount, token_program)
}
