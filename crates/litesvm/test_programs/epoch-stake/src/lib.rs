//! Asserts `sol_get_epoch_stake` values configured on the LiteSVM host.
//!
//! Instruction data layout (48 bytes):
//! - `vote_address`: 32 bytes
//! - `expected_vote_stake`: u64 LE
//! - `expected_total_stake`: u64 LE

use {
    solana_account_info::AccountInfo, solana_epoch_stake, solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

solana_program_entrypoint::entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    assert_eq!(instruction_data.len(), 48, "expected 48-byte ix data");

    let vote_address = Pubkey::new_from_array(instruction_data[0..32].try_into().unwrap());
    let expected_vote_stake = u64::from_le_bytes(instruction_data[32..40].try_into().unwrap());
    let expected_total_stake = u64::from_le_bytes(instruction_data[40..48].try_into().unwrap());

    let vote_stake = solana_epoch_stake::get_epoch_stake_for_vote_account(&vote_address);
    let total_stake = solana_epoch_stake::get_epoch_total_stake();

    assert_eq!(vote_stake, expected_vote_stake);
    assert_eq!(total_stake, expected_total_stake);
    Ok(())
}
