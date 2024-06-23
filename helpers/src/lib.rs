#![allow(clippy::result_large_err)]
use solana_sdk::signature::Keypair;

pub mod loader;
pub mod spl;

pub fn keypair_clone(kp: &Keypair) -> Keypair {
    Keypair::from_bytes(&kp.to_bytes()).expect("failed to copy keypair")
}
