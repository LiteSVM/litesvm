// copied from agave commit 63b13a1f6ad263fb62e1f80156eaf09838f1aff0
// with some execute_timings usage removed
use {
    solana_instruction_error::InstructionError,
    solana_program_runtime::invoke_context::InvokeContext, solana_svm_timings::ExecuteTimings,
    solana_svm_transaction::svm_message::SVMMessage, solana_transaction_context::IndexOfAccount,
    solana_transaction_error::TransactionError,
};

/// Called right before a top-level instruction executes.
pub(crate) type BeforeInstructionHook<'h> = dyn FnMut(usize, &mut InvokeContext) + 'h;

/// Called right after a top-level instruction executes, with its result and
/// the compute units it consumed. The `InvokeContext` still holds the
/// transaction's account state at this point, so the hook can observe every
/// account exactly as it stands between instructions.
pub(crate) type AfterInstructionHook<'h> =
    dyn FnMut(usize, &InvokeContext, &Result<(), InstructionError>, u64) + 'h;

/// Process a message.
/// This method calls each instruction in the message over the set of loaded accounts.
/// For each instruction it calls the program entrypoint method and verifies that the result of
/// the call does not violate the bank's accounting rules.
/// The accounts are committed back to the bank only if every instruction succeeds.
pub(crate) fn process_message<'ix_data>(
    message: &'ix_data impl SVMMessage,
    program_indices: &[IndexOfAccount],
    invoke_context: &mut InvokeContext<'_, 'ix_data>,
    execute_timings: &mut ExecuteTimings,
    accumulated_consumed_units: &mut u64,
    before_instruction: &mut BeforeInstructionHook<'_>,
    after_instruction: &mut AfterInstructionHook<'_>,
) -> Result<(), TransactionError> {
    debug_assert_eq!(program_indices.len(), message.num_instructions());
    invoke_context
        .prepare_top_level_instructions(message)
        .map_err(|(instruction_index, err)| {
            TransactionError::InstructionError(instruction_index, err)
        })?;

    for (top_level_instruction_index, ((program_id, instruction), _program_account_index)) in
        message
            .program_instructions_iter()
            .zip(program_indices.iter())
            .enumerate()
    {
        let mut compute_units_consumed = 0;
        before_instruction(top_level_instruction_index, invoke_context);
        let result = if invoke_context.is_precompile(program_id) {
            invoke_context.process_precompile(
                program_id,
                instruction.data,
                message.instructions_iter().map(|ix| ix.data),
            )
        } else {
            invoke_context.process_instruction(&mut compute_units_consumed, execute_timings)
        };
        after_instruction(
            top_level_instruction_index,
            invoke_context,
            &result,
            compute_units_consumed,
        );

        *accumulated_consumed_units =
            accumulated_consumed_units.saturating_add(compute_units_consumed);

        result.map_err(|err| {
            TransactionError::InstructionError(top_level_instruction_index as u8, err)
        })?;
    }
    Ok(())
}
