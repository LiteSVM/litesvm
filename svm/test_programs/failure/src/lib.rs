// This program just returns an error.

use solana_program::entrypoint;
use solana_program::{
    account_info::AccountInfo, declare_id, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
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
