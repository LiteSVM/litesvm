use {
    borsh::{BorshDeserialize, BorshSerialize},
    ephemeral_rollups_sdk::{
        cpi::{delegate_account, undelegate_account, DelegateAccounts, DelegateConfig},
        ephem::{FoldableIntentBuilder, MagicIntentBundleBuilder},
    },
    solana_account_info::{next_account_info, AccountInfo},
    solana_msg::msg,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::{declare_id, Pubkey},
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Counter {
    pub count: u32,
}

declare_id!("DCntr1hZ6D66VJwY9WQ8UXux1Jdd7EavqrRztdr7RrQk");

const COUNTER_SEED: &[u8] = b"counter";
const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

#[cfg(not(feature = "no-entrypoint"))]
use solana_program_entrypoint::entrypoint;

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.starts_with(&EXTERNAL_UNDELEGATE_DISCRIMINATOR) {
        msg!("Instruction: ExternalUndelegateDelegatedCounter");
        return process_external_undelegate(accounts, &instruction_data[8..]);
    }

    let Some((instruction_discriminant, instruction_data_inner)) = instruction_data.split_first()
    else {
        msg!("Error: missing instruction discriminant");
        return Err(ProgramError::InvalidInstructionData);
    };

    match instruction_discriminant {
        0 => {
            msg!("Instruction: IncrementCounter");
            process_increment_counter(accounts, instruction_data_inner)?;
        }
        1 => {
            msg!("Instruction: DelegateCounter");
            process_delegate_counter(accounts)?;
        }
        2 => {
            msg!("Instruction: UndelegateCounter");
            process_undelegate_counter(accounts)?;
        }
        _ => {
            msg!("Error: unknown instruction");
            return Err(ProgramError::InvalidInstructionData);
        }
    }
    Ok(())
}

pub fn process_increment_counter(
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let account_info_iter = &mut accounts.iter();

    let counter_account = next_account_info(account_info_iter)?;
    if !counter_account.is_writable {
        msg!("Counter account must be writable");
        return Err(ProgramError::InvalidAccountData);
    }

    let mut counter = Counter::try_from_slice(&counter_account.try_borrow_data()?)?;
    counter.count += 1;
    counter.serialize(&mut *counter_account.data.borrow_mut())?;

    msg!("Delegated counter state incremented to {:?}", counter.count);
    Ok(())
}

fn process_delegate_counter(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let payer = next_account_info(account_info_iter)?;
    let counter_account = next_account_info(account_info_iter)?;
    let owner_program = next_account_info(account_info_iter)?;
    let buffer = next_account_info(account_info_iter)?;
    let delegation_record = next_account_info(account_info_iter)?;
    let delegation_metadata = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let delegation_program = next_account_info(account_info_iter)?;

    assert_counter_pda(counter_account)?;

    let pda_seeds: &[&[u8]] = &[COUNTER_SEED];

    delegate_account(
        DelegateAccounts {
            payer,
            pda: counter_account,
            owner_program,
            buffer,
            delegation_record,
            delegation_metadata,
            delegation_program,
            system_program,
        },
        pda_seeds,
        DelegateConfig::default(),
    )
}

fn process_undelegate_counter(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let payer = next_account_info(account_info_iter)?;
    let counter_account = next_account_info(account_info_iter)?;
    let magic_program = next_account_info(account_info_iter)?;
    let magic_context = next_account_info(account_info_iter)?;

    MagicIntentBundleBuilder::new(payer.clone(), magic_context.clone(), magic_program.clone())
        .commit_and_undelegate(&[counter_account.clone()])
        .build_and_invoke()
}

fn process_external_undelegate(accounts: &[AccountInfo], ix_data: &[u8]) -> ProgramResult {
    let [counter_info, buffer_acc, payer_info, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let account_signer_seeds = borsh::from_slice::<Vec<Vec<u8>>>(ix_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    undelegate_account(
        counter_info,
        &crate::ID,
        buffer_acc,
        payer_info,
        system_program,
        account_signer_seeds,
    )?;
    Ok(())
}

fn assert_counter_pda(counter_account: &AccountInfo) -> Result<u8, ProgramError> {
    let (expected_counter, counter_bump) = Pubkey::find_program_address(&[COUNTER_SEED], &id());
    if counter_account.key != &expected_counter {
        msg!("Invalid delegated counter PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(counter_bump)
}
