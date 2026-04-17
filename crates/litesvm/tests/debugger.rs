#[cfg(feature = "sbpf-debugger")]
use {
    agave_feature_set::FeatureSet,
    litesvm::{
        debugger::{
            stub_connect, stub_fetch_debug_metadata, stub_read_memory_chunked, stub_read_register,
            stub_send_continue_command,
        },
        register_tracing::{compute_hash, DefaultRegisterTracingCallback},
        LiteSVM,
    },
    solana_address::address,
    solana_keypair::Keypair,
    solana_message::{AccountMeta, Instruction, Message},
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::PathBuf,
    },
};

#[cfg(feature = "sbpf-debugger")]
fn read_program(so_path: &str) -> Vec<u8> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(so_path);
    std::fs::read(path).unwrap()
}

#[cfg(feature = "sbpf-debugger")]
#[test]
pub fn test_cpi_with_debugger() {
    const SBF_DEBUG_PORT: u16 = 21212;
    const STUB_ADDR: SocketAddr =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), SBF_DEBUG_PORT);
    const STUB_CONNECT_RETRIES: usize = 30;
    const SBF_TRACE_DIR: &str = "target/sbf/trace";

    let enable_register_tracing = true;
    let mut feature_set = FeatureSet::default();
    feature_set.activate(
        &agave_feature_set::provide_instruction_data_offset_in_vm_r2::id(),
        0,
    );
    let mut svm = LiteSVM::new_debuggable(enable_register_tracing).with_feature_set(feature_set);

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let unassociated_program_id = address!("CPPyHetBfV5Xy4xrfiki9FyMjmp5qh5dKNJBaMkvRp1P");
    let cpi_maker_program_id = address!("4fXuRQH9Xd7aZ25MG1nwDZhb9WNBC4bMfYE2AJTWTnR1");
    let cpi_target_program_id = address!("HAnysC5mLjYWPhSYMDyp31WzdCxmMDaij2Bkts9doedP");

    svm.add_program(
        cpi_maker_program_id,
        &read_program("test_programs/target/deploy/test_program_cpi_maker.so"),
    )
    .unwrap();
    svm.add_program(
        cpi_target_program_id,
        &read_program("test_programs/target/deploy/litesvm_clock_example.so"),
    )
    .unwrap();

    svm.airdrop(&payer_pk, 1000000000).unwrap();

    let data = cpi_target_program_id.to_bytes().to_vec();
    let instruction_data_len = data.len();
    let instruction = Instruction {
        program_id: cpi_maker_program_id,
        accounts: vec![AccountMeta::new(cpi_target_program_id, false)],
        data,
    };

    // Phase 1. Test with sbf_trace_filters that will actually invoke the debugger.

    // Empty means it will invoke the debugger as long as the debugger port is set.
    let empty_filter_will_invoke_debugger_if_port_is_set = "".to_string();
    // Specific one, for the program_ids we just prepared - the CPI maker's and the target program's.
    let specific_filter =
        format!("program_id == {cpi_maker_program_id} || program_id == {cpi_target_program_id}");

    let test_trace_filters = [
        empty_filter_will_invoke_debugger_if_port_is_set,
        specific_filter,
    ];

    for sbf_trace_filter in test_trace_filters.into_iter() {
        svm.expire_blockhash();
        svm.set_invocation_inspect_callback(DefaultRegisterTracingCallback {
            sbf_trace_dir: SBF_TRACE_DIR.into(),
            sbf_trace_disassemble: false,
            sbf_debug_port: SBF_DEBUG_PORT.into(),
            sbf_trace_filter,
        });
        let blockhash = svm.latest_blockhash();

        let program_id_file = std::path::PathBuf::from(SBF_TRACE_DIR)
            .join("program_ids")
            .with_extension("map");

        // This is the expected program IDs <-> SHA-256 mapping.
        let expected_program_ids = format!(
            "{}={}\n{}={}\n",
            cpi_maker_program_id,
            compute_hash(
                svm.accounts_db()
                    .try_program_elf_bytes(&cpi_maker_program_id)
                    .unwrap()
            ),
            cpi_target_program_id,
            compute_hash(
                svm.accounts_db()
                    .try_program_elf_bytes(&cpi_target_program_id)
                    .unwrap()
            ),
        );

        // Execute the TX that does a CPI.
        // It's supposed to hang waiting for a TCP connection on the debugger port.
        std::thread::scope(|s| {
            let client_jh = s.spawn(|| -> Result<(), std::io::Error> {
                // Connect to the debugger stub.
                let (mut reader, mut writer) = stub_connect(STUB_ADDR, STUB_CONNECT_RETRIES)?;

                // Check r2 - it should point to the instruction data whereas the length is 8
                // bytes prior to it.
                let data_addr = stub_read_register(&mut writer, &mut reader, 2)?;
                let data_len = u64::from_le_bytes(
                    stub_read_memory_chunked(&mut writer, &mut reader, data_addr - 8, 8, 1024)?
                        .try_into()
                        .map_err(|_| std::io::Error::other("expected 8 bytes"))?,
                ) as usize;
                assert!(instruction_data_len == data_len);
                let data =
                    stub_read_memory_chunked(&mut writer, &mut reader, data_addr, data_len, 1024)?;
                assert!(instruction.data == data);

                let parsed_map = stub_fetch_debug_metadata(&mut reader, &mut writer)?;

                // After parsing the reply check the runtime has passed to us the
                // expected program_id in the metadata.
                assert!(
                    parsed_map.get("program_id") == Some(&cpi_maker_program_id.to_string())
                        && parsed_map.get("cpi_level") == Some(&"0".to_string())
                        && parsed_map.get("caller") == Some(&"none".to_string())
                );

                // Fire the CPI handling prior to issuing the continue command.
                let cpi_client_jh = s.spawn(|| -> Result<(), std::io::Error> {
                    // The CPI means we have another gdb stub instantiated and listening.
                    let (mut reader, mut writer) = stub_connect(STUB_ADDR, STUB_CONNECT_RETRIES)?;

                    let parsed_map = stub_fetch_debug_metadata(&mut reader, &mut writer)?;

                    // Check the CPI callee and caller and level.
                    assert!(
                        parsed_map.get("program_id") == Some(&cpi_target_program_id.to_string())
                            && parsed_map.get("cpi_level") == Some(&"1".to_string())
                            && parsed_map.get("caller") == Some(&cpi_maker_program_id.to_string())
                    );

                    // Issue the continue command.
                    stub_send_continue_command(&mut reader, &mut writer)?;

                    Ok(())
                });

                // Issue the continue command.
                stub_send_continue_command(&mut reader, &mut writer)?;

                cpi_client_jh.join().unwrap().expect("cpi client error");

                Ok(())
            });

            // Processing...
            let msg =
                Message::new_with_blockhash(&[instruction.clone()], Some(&payer_pk), &blockhash);
            let tx = Transaction::new(&[&payer_kp], msg, blockhash);
            let _meta = svm.send_transaction(tx).unwrap();

            client_jh.join().unwrap().expect("client error");
        });

        // Check the program_ids <-> elf sha256 mapping table.
        let read_program_ids = std::fs::read_to_string(&program_id_file).unwrap();
        let mut read_lines: Vec<&str> = read_program_ids.lines().collect();
        let mut expected_lines: Vec<&str> = expected_program_ids.lines().collect();
        read_lines.sort();
        expected_lines.sort();
        assert_eq!(read_lines, expected_lines);
    }

    // Phase 2 - try to debug inexisting program_id, the gdbstub must not hang even
    // if the debug_port is set.

    let not_matching_filter = format!("program_id == {unassociated_program_id}");
    svm.expire_blockhash();
    svm.set_invocation_inspect_callback(DefaultRegisterTracingCallback {
        sbf_trace_dir: SBF_TRACE_DIR.into(),
        sbf_trace_disassemble: false,
        sbf_debug_port: SBF_DEBUG_PORT.into(),
        sbf_trace_filter: not_matching_filter,
    });
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction.clone()], Some(&payer_pk), &blockhash);
    let tx = Transaction::new(&[&payer_kp], msg, blockhash);
    let _meta = svm.send_transaction(tx).unwrap();
}
