// copied from agave commit 63b13a1f6ad263fb62e1f80156eaf09838f1aff0
// with some execute_timings usage removed
use {
    jupnet_program_runtime::invoke_context::InvokeContext,
    jupnet_sdk::{
        account::WritableAccount,
        sysvar::instructions,
        transaction_context::{IndexOfAccount, InstructionAccount},
    },
    jupnet_svm_transaction::svm_message::SVMExecutionMessage,
    jupnet_timings::ExecuteTimings,
    jupnet_transaction_error::TransactionError,
};

/// Process a message.
/// This method calls each instruction in the message over the set of loaded accounts.
/// For each instruction it calls the program entrypoint method and verifies that the result of
/// the call does not violate the bank's accounting rules.
/// The accounts are committed back to the bank only if every instruction succeeds.
pub(crate) fn process_message(
    message: &SVMExecutionMessage,
    program_indices: &[Vec<IndexOfAccount>],
    invoke_context: &mut InvokeContext,
    execute_timings: &mut ExecuteTimings,
    accumulated_consumed_units: &mut u64,
) -> Result<(), TransactionError> {
    for (instruction_index, (instruction_info, program_indices)) in message
        .instructions
        .iter()
        .zip(program_indices.iter())
        .enumerate()
    {
        // Fixup the special instructions key if present
        // before the account pre-values are taken care of
        if let Some(account_index) = invoke_context
            .transaction_context
            .find_index_of_account(&instructions::id())
        {
            let mut mut_account_ref = invoke_context
                .transaction_context
                .get_account_at_index(account_index)
                .map_err(|_| TransactionError::InvalidAccountIndex)?
                .borrow_mut();
            instructions::store_current_index(
                mut_account_ref.data_as_mut_slice(),
                instruction_index as u16,
            );
        }

        let mut instruction_accounts = Vec::with_capacity(instruction_info.accounts.len());
        for (instruction_account_index, index_in_transaction) in
            instruction_info.accounts.iter().enumerate()
        {
            let index_in_callee = instruction_info
                .accounts
                .get(0..instruction_account_index)
                .ok_or(TransactionError::InvalidAccountIndex)?
                .iter()
                .position(|account_index| account_index == index_in_transaction)
                .unwrap_or(instruction_account_index)
                as IndexOfAccount;
            let index_in_transaction = *index_in_transaction;
            instruction_accounts.push(InstructionAccount {
                index_in_transaction: index_in_transaction as IndexOfAccount,
                index_in_caller: index_in_transaction as IndexOfAccount,
                index_in_callee,
                is_signer: instruction_info.is_signer[instruction_account_index],
                is_writable: instruction_info.is_writable[instruction_account_index],
            });
        }

        let mut compute_units_consumed = 0;
        let result = invoke_context.process_instruction(
            instruction_info.data,
            &instruction_accounts,
            program_indices,
            &mut compute_units_consumed,
            execute_timings,
        );

        *accumulated_consumed_units =
            accumulated_consumed_units.saturating_add(compute_units_consumed);

        result.map_err(|err| TransactionError::InstructionError(instruction_index as u8, err))?;
    }
    Ok(())
}
