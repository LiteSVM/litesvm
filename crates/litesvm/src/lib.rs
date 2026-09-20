//! A fast, lightweight Solana VM simulator.
//!
//! Enable the `fixture` feature for the fixture-based test harness.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use litesvm_core::*;

#[cfg(feature = "fixture")]
#[cfg_attr(docsrs, doc(cfg(feature = "fixture")))]
pub use litesvm_fixture as fixture;
