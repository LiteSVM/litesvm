use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::{address, Address},
    solana_address_lookup_table_interface::instruction::{
        create_lookup_table, extend_lookup_table,
    },
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{
        v0::Message as MessageV0, AddressLookupTableAccount, Message, VersionedMessage,
    },
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    solana_transaction_error::TransactionError,
    std::path::PathBuf,
};

fn read_counter_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    std::fs::read(so_path).unwrap()
}

#[test]
pub fn integration_test() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = address!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_counter_program())
        .unwrap();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = address!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
    let _ = svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    );
    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        0u32.to_le_bytes().to_vec()
    );
    let num_greets = 2u8;
    for deduper in 0..num_greets {
        let tx = make_tx(
            program_id,
            counter_address,
            &payer_pk,
            blockhash,
            &payer_kp,
            deduper,
        );
        let _ = svm.send_transaction(tx).unwrap();
    }
    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        (num_greets as u32).to_le_bytes().to_vec()
    );
}

fn make_tx(
    program_id: Address,
    counter_address: Address,
    payer_pk: &Address,
    blockhash: solana_hash::Hash,
    payer_kp: &Keypair,
    deduper: u8,
) -> Transaction {
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0, deduper],
        }],
        Some(payer_pk),
        &blockhash,
    );
    Transaction::new(&[payer_kp], msg, blockhash)
}

#[test]
fn test_address_lookup_table() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = address!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_counter_program())
        .unwrap();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = address!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
    let _ = svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    );
    let (lookup_table_ix, lookup_table_address) = create_lookup_table(payer_pk, payer_pk, 0);
    let extend_ix = extend_lookup_table(
        lookup_table_address,
        payer_pk,
        Some(payer_pk),
        vec![counter_address],
    );
    let lookup_msg = Message::new(&[lookup_table_ix, extend_ix], Some(&payer_pk));
    let lookup_tx = Transaction::new(&[&payer_kp], lookup_msg, blockhash);
    svm.send_transaction(lookup_tx).unwrap();
    let alta = AddressLookupTableAccount {
        key: lookup_table_address,
        addresses: vec![counter_address],
    };
    let counter_msg = MessageV0::try_compile(
        &payer_pk,
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0, 0],
        }],
        &[alta],
        blockhash,
    )
    .unwrap();
    let counter_tx =
        VersionedTransaction::try_new(VersionedMessage::V0(counter_msg), &[&payer_kp]).unwrap();
    svm.warp_to_slot(1); // can't use the lookup table in the same slot
    svm.send_transaction(counter_tx).unwrap();
}

#[test]
pub fn test_nonexistent_program() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = address!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = address!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
    svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    )
    .unwrap();
    let tx = make_tx(
        program_id,
        counter_address,
        &payer_pk,
        blockhash,
        &payer_kp,
        0,
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert_eq!(err.err, TransactionError::InvalidProgramForExecution);
}

#[cfg(feature = "register-tracing")]
// In order to test register tracing we need a SBF program to invoke.
// The counter program is a good candidate for this purpose.
#[test]
fn test_register_tracing_handler() {
    use {
        litesvm::InvocationInspectCallback,
        solana_program_runtime::invoke_context::{Executable, InvokeContext, RegisterTrace},
        solana_transaction::{sanitized::SanitizedTransaction, Address},
        solana_transaction_context::{IndexOfAccount, InstructionContext},
        std::{
            collections::HashMap,
            sync::{Arc, Mutex},
        },
    };

    let enable_register_tracing = true;

    let mut svm = LiteSVM::new_debuggable(enable_register_tracing);

    struct TracingData {
        program_id: Address,
        executed_jump_instructions_count: usize,
    }

    struct CustomRegisterTracingCallback {
        tracing_data: Arc<Mutex<HashMap<Address, TracingData>>>,
    }

    impl CustomRegisterTracingCallback {
        pub fn handler(
            &self,
            instruction_context: InstructionContext,
            executable: &Executable,
            register_trace: RegisterTrace,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let mut tracing_data = self.tracing_data.lock().unwrap();

            let program_id = instruction_context.get_program_key().unwrap();
            let (_vm_addr, program) = executable.get_text_bytes();
            let executed_jump_instructions_count = register_trace
                .iter()
                .map(|registers| {
                    (
                        registers,
                        solana_program_runtime::solana_sbpf::ebpf::get_insn_unchecked(
                            program,
                            registers[11] as usize,
                        ),
                    )
                })
                .filter(|(_registers, insn)| {
                    insn.opc & 7 == solana_program_runtime::solana_sbpf::ebpf::BPF_JMP
                        && insn.opc != solana_program_runtime::solana_sbpf::ebpf::BPF_JA
                })
                .count();
            let entry = tracing_data.entry(*program_id).or_insert(TracingData {
                program_id: *program_id,
                executed_jump_instructions_count: 0,
            });
            entry.executed_jump_instructions_count = entry
                .executed_jump_instructions_count
                .saturating_add(executed_jump_instructions_count);

            Ok(())
        }
    }

    impl InvocationInspectCallback for CustomRegisterTracingCallback {
        fn before_invocation(
            &self,
            _: &SanitizedTransaction,
            _: &[IndexOfAccount],
            _: &InvokeContext,
        ) {
        }

        fn after_invocation(&self, invoke_context: &InvokeContext, register_tracing_enabled: bool) {
            // Only process traces if register tracing was enabled.
            if register_tracing_enabled {
                invoke_context.iterate_vm_traces(
                    &|instruction_context: InstructionContext,
                      executable: &Executable,
                      register_trace: RegisterTrace| {
                        if let Err(e) =
                            self.handler(instruction_context, executable, register_trace)
                        {
                            eprintln!("Error collecting the register tracing: {}", e);
                        }
                    },
                );
            }
        }
    }

    // Phase 1 - basic register tracing test.

    // Have a custom register tracing handler counting the total number of executed
    // jump instructions per program_id.
    let tracing_data = Arc::new(Mutex::new(HashMap::<Address, TracingData>::new()));
    svm.set_invocation_inspect_callback(CustomRegisterTracingCallback {
        tracing_data: Arc::clone(&tracing_data),
    });

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = address!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");

    let init_svm = |svm: &mut LiteSVM| -> Address {
        svm.add_program(program_id, &read_counter_program())
            .unwrap();
        svm.airdrop(&payer_pk, 1000000000).unwrap();
        let counter_address = address!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
        let _ = svm.set_account(
            counter_address,
            Account {
                lamports: 5,
                data: vec![0_u8; std::mem::size_of::<u32>()],
                owner: program_id,
                ..Default::default()
            },
        );
        counter_address
    };
    let counter_address = init_svm(&mut svm);
    let blockhash = svm.latest_blockhash();
    let tx = make_tx(
        program_id,
        counter_address,
        &payer_pk,
        blockhash,
        &payer_kp,
        0,
    );
    let _ = svm.send_transaction(tx).unwrap();

    let executed_jump_instruction_count_from_phase1;
    // Let's check the outcome of the custom register tracing callback.
    {
        assert_eq!(tracing_data.lock().unwrap().len(), 1);
        let td = tracing_data.lock().unwrap();
        let collected_data = td.get(&program_id).unwrap();

        // Check it's the program_id only on our list.
        assert_eq!(collected_data.program_id, program_id);
        // Check the number of executed jump class instructions is greater than 0.
        assert!(collected_data.executed_jump_instructions_count > 0);

        // Store this value for a later comparison.
        executed_jump_instruction_count_from_phase1 =
            collected_data.executed_jump_instructions_count;
    }

    // Phase 2 - check that register tracing is disabled when constructing
    // LiteSVM with enable_register_tracing=false.
    {
        // Clear the tracing data collected so far.
        {
            let mut td = tracing_data.lock().unwrap();
            td.clear();
        }

        // Create a new LiteSVM instance with register tracing disabled.
        let mut svm_no_tracing = LiteSVM::new_debuggable(/* enable_register_tracing */ false);
        let counter_address = init_svm(&mut svm_no_tracing);
        svm_no_tracing.set_invocation_inspect_callback(CustomRegisterTracingCallback {
            tracing_data: Arc::clone(&tracing_data),
        });

        // Execute the same transaction again.
        let blockhash = svm_no_tracing.latest_blockhash();
        let tx = make_tx(
            program_id,
            counter_address,
            &payer_pk,
            blockhash,
            &payer_kp,
            0,
        );
        let _ = svm_no_tracing.send_transaction(tx).unwrap();

        let td = tracing_data.lock().unwrap();
        // We expect it to be empty since tracing was disabled!
        assert!(td.is_empty());
    }

    // Phase 3 - check we can have register tracing enabled for a new instance of
    // LiteSVM.
    {
        // Create a new LiteSVM instance with register tracing enabled.
        let mut svm_with_tracing = LiteSVM::new_debuggable(/* enable_register_tracing */ true);
        let counter_address = init_svm(&mut svm_with_tracing);
        svm_with_tracing.set_invocation_inspect_callback(CustomRegisterTracingCallback {
            tracing_data: Arc::clone(&tracing_data),
        });

        // Execute the same transaction again.
        let blockhash = svm_with_tracing.latest_blockhash();
        let tx = make_tx(
            program_id,
            counter_address,
            &payer_pk,
            blockhash,
            &payer_kp,
            0,
        );
        let _ = svm_with_tracing.send_transaction(tx).unwrap();

        let td = tracing_data.lock().unwrap();
        let collected_data = td.get(&program_id).unwrap();

        // Check again it's the program_id only on our list.
        assert_eq!(collected_data.program_id, program_id);
        // Check the number of executed jump instructions is the same as we did in
        // phase 1 of this test.
        assert!(
            collected_data.executed_jump_instructions_count
                == executed_jump_instruction_count_from_phase1
        );
    }
}
