use thiserror::Error;

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("write error: {0}")]
    Write(#[from] wincode::error::WriteError),
    #[error("read error: {0}")]
    Read(#[from] wincode::error::ReadError),
    #[error("empty input")]
    EmptyInput,
    #[error("unsupported snapshot version: {0}")]
    UnsupportedVersion(u8),
    #[error("failed to rebuild caches: {0}")]
    CacheRebuild(#[from] litesvm::error::LiteSVMError),
    #[error("serialization thread panicked")]
    ThreadPanic,
}
