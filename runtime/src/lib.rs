use thiserror::Error;

pub mod bank;
pub mod types;

mod accounts_db;
mod builtin;
mod utils;

pub use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;
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
