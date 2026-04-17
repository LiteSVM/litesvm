#![cfg(target_os = "solana")]

use {
    solana_account_info::{next_account_info, AccountInfo},
    solana_instruction::{AccountMeta, Instruction},
    solana_msg::msg,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

solana_program_entrypoint::entrypoint!(process_instruction);

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> Result<(), ProgramError> {
    let mut accounts_iter = accounts.iter();
    let account_info = next_account_info(&mut accounts_iter)?;

    let target_program_id =
        Pubkey::try_from(&input[..32]).map_err(|_| ProgramError::InvalidInstructionData)?;

    msg!("Making CPI into {}", target_program_id);
    solana_cpi::invoke(
        &Instruction::new_with_bytes(
            target_program_id,
            &[],
            vec![AccountMeta::new(*account_info.key, false)],
        ),
        std::slice::from_ref(account_info),
    )?;

    Ok(())
}
