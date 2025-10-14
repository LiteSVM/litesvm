use thiserror::Error;

#[derive(Error, Debug)]
pub enum SodaSVMError {
    #[error("LiteSVM error: {0}")]
    LiteSVM(#[from] litesvm::error::LiteSVMError),
    #[error("Transaction failed: {0}")]
    Transaction(String),
}