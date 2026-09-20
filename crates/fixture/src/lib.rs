//! Parallax: a fixture-based testing harness for Solana programs on LiteSVM.
//!
//! [`parallax_test`] turns an ordinary Rust test into an isolated [`Ctx`]
//! world loaded with the current program. [`fixture`] provides composable
//! account setup, while [`Outcome`] keeps execution assertions structured and
//! independent of the SVM that ran the transaction. Typed account state is
//! read and written with [wincode](https://docs.rs/wincode) — a serialization
//! standard, not a framework — so the harness stays program-agnostic.
//!
//! ```rust,ignore
//! use litesvm::fixture::prelude::*;
//!
//! #[parallax_test]
//! fn initializes(test: &mut Ctx) {
//!     let authority = test.add(Wallet::account());
//!     ctx.execute(InitializeInstruction { authority })
//!         .check(Outcome::success());
//! }
//! ```
//!
//! [`fixture::Wallet::account`] funds an actor with the default balance;
//! [`fixture::Wallet::fund`] sets an exact one. Any signer a transaction names
//! but never installs is auto-funded on send, so co-signers cost nothing extra.
//!
//! The name is the pitch: the same program observed from multiple vantage
//! points — this Rust crate and the `parallax-svm` TypeScript package (Kit and
//! Web3.js) — must agree. Fixture addresses are deterministic and identical
//! across all three.
//!
//! # Determinism
//!
//! Execution is deterministic and is treated as a guarantee, not an accident:
//! two fresh [`Ctx`] worlds running the identical scenario produce
//! byte-identical results — the same [`Outcome`] (error, compute units, logs,
//! return data, account changes) and the same post-state account bytes. The
//! backend seeds a fixed genesis blockhash and a zero-timestamp clock (no
//! wall-clock reads, no RNG), fixture placement follows a per-world
//! deterministic address sequence, and every observable ordering — the tracked
//! account set, the outcome's accounts and changes — is first-appearance, never
//! hash-map iteration. A run therefore reproduces exactly, in this crate and
//! across the TypeScript harnesses. (Both harness test suites assert this
//! directly against two worlds.)

#![warn(missing_docs)]

mod accounts;
mod backend;
mod check;
mod dump;
pub mod fixture;
mod outcome;
mod setup;
mod types;
mod world;

pub use {
    check::{
        bundle, CheckFn, Cu, DataExpected, Expected, ExpectedBytes, IntoTransactionError, Raw,
        ReturnData, Typed,
    },
    litesvm_fixture_derive::parallax_test,
    outcome::Outcome,
    setup::{CtxBuilder, SetupError, PROGRAM_PATH_ENV},
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::Pubkey,
    solana_sdk_ids::system_program,
    types::{Account, AccountChange, ProgramError},
    world::{Ctx, Snapshot, DEFAULT_WALLET_LAMPORTS},
    world::{IntoInstructions, Many, One},
};

/// The SPL Token program.
pub const SPL_TOKEN_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// The Token-2022 program.
pub const SPL_TOKEN_2022_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// The SPL Associated Token Account program.
pub const SPL_ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// Build read-only signer metas for co-signers, such as multisig members.
///
/// Each address becomes an [`AccountMeta`] that is read-only and a signer, the
/// shape a program expects for an authority it only needs to have signed.
/// [`Ctx::execute`] auto-registers any co-signer the world has not installed as a
/// funded system account — as it does for every signer it backfills — so tests
/// pass the addresses alone without hand-rolling metas or wallets.
pub fn co_signers(addresses: &[Pubkey]) -> Vec<AccountMeta> {
    addresses
        .iter()
        .map(|&address| AccountMeta::new_readonly(address, true))
        .collect()
}

/// Imports used by most program tests.
pub mod prelude {
    pub use crate::{
        bundle, co_signers,
        fixture::{
            AssociatedTokenAccount, Dump, Fixture, Load, Mint, Program, TokenAccount, TokenProgram,
            Wallet,
        },
        parallax_test, system_program, Account, AccountChange, AccountMeta, CheckFn, Ctx, Cu,
        Instruction, Outcome, ProgramError, Pubkey, ReturnData, Snapshot, DEFAULT_WALLET_LAMPORTS,
        SPL_ASSOCIATED_TOKEN_PROGRAM_ID, SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
    };
}

#[cfg(test)]
mod tests {
    use {
        super::{
            backend::Backend,
            dump::DEFAULT_RPC_URL,
            fixture::{Mint, TokenAccount, TokenProgram, Wallet},
            Ctx, Pubkey, SPL_TOKEN_2022_PROGRAM_ID,
        },
        std::path::PathBuf,
    };

    fn empty_ctx() -> Ctx {
        Ctx::from_parts(
            Backend::new(),
            Pubkey::new_from_array([42; 32]),
            PathBuf::new(),
            DEFAULT_RPC_URL.to_string(),
            None,
        )
    }

    #[test]
    fn fixtures_are_deterministic_and_compose() {
        let (mut first, mut second) = (empty_ctx(), empty_ctx());
        let wallet = first.add(Wallet::account().fund(42));
        assert_eq!(wallet, second.add(Wallet::account().fund(42)));

        let mint = first.add(
            Mint::account()
                .with_supply(1_000)
                .token_program(TokenProgram::Token2022),
        );
        let tokens = first.add(
            TokenAccount::account(mint, wallet)
                .with_amount(600)
                .token_program(TokenProgram::Token2022),
        );

        assert_eq!(first.lamports(wallet), 42);
        assert_eq!(first.supply(mint), 1_000);
        assert_eq!(first.tokens(tokens), 600);
        assert_eq!(
            first.account(mint).unwrap().owner,
            SPL_TOKEN_2022_PROGRAM_ID
        );
    }
}
