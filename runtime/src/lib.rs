use thiserror::Error;

pub mod bank;
pub mod types;

mod accounts_db;
mod builtin;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InstructionError(#[from] solana_sdk::instruction::InstructionError),
    #[error(transparent)]
    TransactionError(#[from] solana_sdk::transaction::TransactionError),
}
