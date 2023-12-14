use solana_program_runtime::loaded_programs::{
    LoadProgramMetrics, LoadedProgram, ProgramRuntimeEnvironment,
};
use solana_sdk::{
    bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable,
    hash::{Hash, Hasher},
    loader_v4,
    pubkey::Pubkey,
};

mod loader;
mod rent;

pub use loader::*;
pub(crate) use rent::*;

use crate::bank::LightBank;

/// Create a blockhash from the given bytes
pub fn create_blockhash(bytes: &[u8]) -> Hash {
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
];

pub(crate) fn _load_program(
    bank: LightBank,
    program_bytes: &[u8],
    program_size: usize,
    runtime: ProgramRuntimeEnvironment,
    loader_key: &Pubkey,
    reload: bool,
) -> LoadedProgram {
    let metrics = &mut LoadProgramMetrics::default();
    if reload {
        // SAFE because program is already verified
        unsafe {
            LoadedProgram::reload(
                loader_key,
                runtime,
                bank.slot(),
                bank.slot(),
                None,
                program_bytes,
                program_size,
                metrics,
            )
        }
        .unwrap_or_default()
    } else {
        LoadedProgram::new(
            loader_key,
            runtime,
            bank.slot(),
            bank.slot(),
            None,
            program_bytes,
            program_size,
            metrics,
        )
        .unwrap_or_default()
    }
}

// pub(crate) fn load_bpf_upgradeable_program() {
//     load_program(
//         bank,
//         program_bytes,
//         program_size,
//         runtime,
//         loader_key,
//         reload,
//     )
// }
