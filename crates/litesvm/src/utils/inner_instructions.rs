use {
    solana_instruction::TRANSACTION_LEVEL_STACK_HEIGHT,
    solana_message::{
        compiled_instruction::CompiledInstruction,
        inner_instruction::{InnerInstruction, InnerInstructionsList},
    },
    solana_transaction_context::transaction::TransactionContext,
};

/// Adapted from `solana-svm` crate, `transaction_processor.rs`
/// (`deconstruct_transaction`), reading through the borrowed context instead
/// of consuming it.
pub fn inner_instructions_list_from_instruction_trace(
    transaction_context: &TransactionContext,
) -> InnerInstructionsList {
    debug_assert!(transaction_context
        .get_instruction_context_at_index_in_trace(0)
        .map(|instruction_context| instruction_context.get_stack_height()
            == TRANSACTION_LEVEL_STACK_HEIGHT)
        .unwrap_or(true));
    let trace_length = transaction_context.get_instruction_trace_length();

    // All top-level instructions are configured at the head of the trace;
    // CPIs are appended after them in execution order.
    let mut num_top_level = 0;
    while num_top_level < trace_length {
        match transaction_context.get_instruction_context_at_index_in_trace(num_top_level) {
            Ok(instruction_context)
                if instruction_context.get_stack_height() == TRANSACTION_LEVEL_STACK_HEIGHT =>
            {
                num_top_level += 1;
            }
            _ => break,
        }
    }

    let mut outer_instructions: InnerInstructionsList = vec![Vec::new(); num_top_level];
    // Maps each trace index to its top-level ancestor. Callers always precede
    // callees in the trace, so the ancestor is resolved by the time a callee
    // is visited.
    let mut top_level_ancestor: Vec<usize> = (0..num_top_level).collect();
    for index_in_trace in num_top_level..trace_length {
        let maybe_context =
            transaction_context.get_instruction_context_at_index_in_trace(index_in_trace);
        let ancestor = maybe_context
            .as_ref()
            .ok()
            .and_then(|instruction_context| {
                top_level_ancestor
                    .get(instruction_context.get_index_of_caller())
                    .copied()
            })
            .unwrap_or(usize::MAX);
        top_level_ancestor.push(ancestor);
        let (Ok(instruction_context), Some(inner_instructions)) =
            (maybe_context, outer_instructions.get_mut(ancestor))
        else {
            debug_assert!(false);
            continue;
        };

        let stack_height = u8::try_from(instruction_context.get_stack_height()).unwrap_or(u8::MAX);
        let instruction = CompiledInstruction::new_from_raw_parts(
            instruction_context
                .get_index_of_program_account_in_transaction()
                .unwrap_or_default() as u8,
            instruction_context.get_instruction_data().to_vec(),
            (0..instruction_context.get_number_of_instruction_accounts())
                .map(|instruction_account_index| {
                    instruction_context
                        .get_index_of_instruction_account_in_transaction(instruction_account_index)
                        .unwrap_or_default() as u8
                })
                .collect(),
        );
        inner_instructions.push(InnerInstruction {
            instruction,
            stack_height,
        });
    }
    outer_instructions
}
