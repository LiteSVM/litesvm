// This program just returns an error.

use solana_program_entrypoint::entrypoint;
use {
    solana_account_info::AccountInfo, solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::{declare_id, Pubkey},
};

declare_id!("HvrRMSshMx3itvsyWDnWg2E3cy5h57iMaR7oVxSZJDSA");

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    Err(ProgramError::Custom(0))
}
