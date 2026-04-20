use thiserror::Error;

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] bincode::Error),
    #[error("unsupported snapshot version: {0}")]
    UnsupportedVersion(u8),
    #[error("failed to rebuild caches: {0}")]
    CacheRebuild(#[from] litesvm::error::LiteSVMError),
    #[error("serialization thread panicked")]
    ThreadPanic,
}
