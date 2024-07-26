#![allow(clippy::result_large_err)]

#[cfg(feature = "loader")]
pub mod loader;

#[cfg(any(feature = "token", feature = "token-2022"))]
pub mod spl;
