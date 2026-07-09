#![cfg(target_os = "solana")]

use {
    solana_account_info::AccountInfo, solana_define_syscall::define_syscall,
    solana_program_error::ProgramError, solana_pubkey::Pubkey,
};

// Declare the custom syscall that we expect to be registered.
// This matches the `sol_burn_cus` syscall from the test.
// define_syscall! emits the correct call encoding for both dynamic (SBPFv0)
// and static-syscalls (SBPFv3) targets; a raw `extern "C"` block links as an
// unresolved call-to-self stub under SBPFv3.
define_syscall!(fn sol_burn_cus(to_burn: u64) -> u64);

solana_program_entrypoint::entrypoint!(process_instruction);

fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    input: &[u8],
) -> Result<(), ProgramError> {
    let to_burn = input
        .get(0..8)
        .and_then(|bytes| bytes.try_into().map(u64::from_le_bytes).ok())
        .ok_or(ProgramError::InvalidInstructionData)?;

    // Call the custom syscall to burn CUs.
    unsafe {
        sol_burn_cus(to_burn);
    }

    Ok(())
}
