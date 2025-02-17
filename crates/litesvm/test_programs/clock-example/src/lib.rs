use {
    solana_account_info::AccountInfo,
    solana_clock::Clock,
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
    solana_sysvar::Sysvar,
};

solana_program_entrypoint::entrypoint!(process_instruction);
#[allow(clippy::unnecessary_wraps)]
pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    // Clock
    let got_clock = Clock::get()?;
    assert!(got_clock.unix_timestamp < 100);
    Ok(())
}
