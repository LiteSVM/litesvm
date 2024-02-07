use thiserror::Error;

pub mod types;

mod accounts_db;
mod bank;
mod builtin;
mod spl;
mod utils;

pub use bank::LiteSVM;
pub use utils::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    SignerError(#[from] solana_sdk::signer::SignerError),
    #[error(transparent)]
    InstructionError(#[from] solana_sdk::instruction::InstructionError),
    #[error(transparent)]
    TransactionError(#[from] solana_sdk::transaction::TransactionError),
}
