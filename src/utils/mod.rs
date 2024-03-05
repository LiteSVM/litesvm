use solana_sdk::{
    bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable,
    hash::{Hash, Hasher},
    loader_v4,
    pubkey::Pubkey,
    system_program,
};

mod loader;
mod rent;

pub use loader::*;
pub(crate) use rent::*;

/// Create a blockhash from the given bytes
pub(crate) fn create_blockhash(bytes: &[u8]) -> Hash {
    let mut hasher = Hasher::default();
    hasher.hash(bytes);
    hasher.result()
}

//TODO remove it in the next solana version
pub const PROGRAM_OWNERS: &[Pubkey] = &[
    bpf_loader_upgradeable::id(),
    bpf_loader::id(),
    bpf_loader_deprecated::id(),
    loader_v4::id(),
    system_program::id(),
];
