use {
    litesvm::LiteSVM,
    litesvm_persistence::{from_bytes, load_from_file, save_to_file, to_bytes, PersistenceError},
    solana_address::Address,
    solana_clock::Clock,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn basic_account_round_trip() {
    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    let addr = Address::new_unique();
    let mut account = solana_account::Account::new(42_000, 128, &Address::default());
    account.data = vec![0xAB; 128];
    svm.set_account(addr, account.clone()).unwrap();

    let dir = temp_dir();
    let path = dir.path().join("basic.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    let loaded = restored.get_account(&addr).unwrap();
    assert_eq!(loaded.lamports, 42_000);
    assert_eq!(loaded.data, vec![0xAB; 128]);
}

#[test]
fn multiple_accounts_round_trip() {
    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    let mut addrs = Vec::new();
    for i in 0..10u64 {
        let addr = Address::new_unique();
        let account = solana_account::Account::new(
            (i + 1) * 1_000_000,
            (i as usize + 1) * 32,
            &Address::default(),
        );
        svm.set_account(addr, account).unwrap();
        addrs.push(addr);
    }

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    for (i, addr) in addrs.iter().enumerate() {
        let acc = restored.get_account(addr).unwrap();
        assert_eq!(acc.lamports, (i as u64 + 1) * 1_000_000);
        assert_eq!(acc.data.len(), (i + 1) * 32);
    }
}

#[test]
fn sysvar_round_trip() {
    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    svm.set_sysvar(&Clock {
        slot: 999,
        epoch_start_timestamp: 1_700_000_000,
        epoch: 42,
        leader_schedule_epoch: 43,
        unix_timestamp: 1_700_000_500,
    });

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    let clock: Clock = restored.get_sysvar();
    assert_eq!(clock.slot, 999);
    assert_eq!(clock.unix_timestamp, 1_700_000_500);
    assert_eq!(clock.epoch, 42);
}

#[test]
fn config_round_trip() {
    let svm = LiteSVM::new()
        .with_builtins()
        .with_sysvars()
        .with_sigverify(false)
        .with_blockhash_check(false)
        .with_log_bytes_limit(Some(4096));

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    assert_eq!(restored.get_sigverify(), false);
    assert_eq!(restored.get_blockhash_check(), false);
    assert_eq!(restored.get_log_bytes_limit(), Some(4096));
    assert_eq!(restored.get_compute_budget(), svm.get_compute_budget());
}

#[test]
fn blockhash_round_trip() {
    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    svm.expire_blockhash();
    let hash_before = svm.latest_blockhash();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    assert_eq!(restored.latest_blockhash(), hash_before);
}

#[test]
fn airdrop_keypair_round_trip() {
    let svm = LiteSVM::new().with_builtins().with_sysvars();
    let original_pubkey = svm.airdrop_pubkey();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    assert_eq!(restored.airdrop_pubkey(), original_pubkey);
}

#[test]
fn transaction_history_round_trip() {
    let mut svm = LiteSVM::new()
        .with_builtins()
        .with_sysvars()
        .with_sigverify(false)
        .with_blockhash_check(false)
        .with_transaction_history(100);

    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(
        Address::new_unique(),
        &[],
        vec![AccountMeta::new(kp.pubkey(), true)],
    );
    let msg = Message::new(&[ix], Some(&kp.pubkey()));
    let tx = Transaction::new(&[&kp], msg, svm.latest_blockhash());
    // This will likely fail (no program), but it should still record in history
    let _ = svm.send_transaction(tx);

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    let original_history = svm.transaction_history_entries();
    let restored_history = restored.transaction_history_entries();
    assert_eq!(original_history.len(), restored_history.len());
    for sig in original_history.keys() {
        assert!(restored_history.contains_key(sig));
    }
}

#[test]
fn bytes_round_trip() {
    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    let addr = Address::new_unique();
    let account = solana_account::Account::new(123_456, 64, &Address::default());
    svm.set_account(addr, account).unwrap();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    let loaded = restored.get_account(&addr).unwrap();
    assert_eq!(loaded.lamports, 123_456);
}

#[test]
fn airdrop_works_after_restore() {
    let svm = LiteSVM::new().with_builtins().with_sysvars();
    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();

    let mut restored = load_from_file(&path).unwrap();
    let recipient = Address::new_unique();
    restored.airdrop(&recipient, 5_000_000_000).unwrap();
    assert_eq!(restored.get_balance(&recipient).unwrap(), 5_000_000_000);
}

#[test]
fn send_transaction_after_restore() {
    let mut svm = LiteSVM::new()
        .with_builtins()
        .with_sysvars()
        .with_sigverify(false)
        .with_blockhash_check(false);

    let from_kp = Keypair::new();
    let to = Address::new_unique();
    svm.airdrop(&from_kp.pubkey(), 10_000_000_000).unwrap();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let mut restored = load_from_file(&path).unwrap();

    let ix = solana_system_interface::instruction::transfer(&from_kp.pubkey(), &to, 1_000_000_000);
    let msg = Message::new(&[ix], Some(&from_kp.pubkey()));
    let tx = Transaction::new(&[&from_kp], msg, restored.latest_blockhash());
    restored.send_transaction(tx).unwrap();

    assert_eq!(restored.get_balance(&to).unwrap(), 1_000_000_000);
}

#[test]
fn bpf_program_round_trip() {
    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    let program_bytes = include_bytes!("../../node-litesvm/program_bytes/spl_example_logging.so");
    let program_id = Address::new_unique();
    svm.add_program(program_id, program_bytes).unwrap();

    // Execute the program before save
    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 1_000_000_000).unwrap();
    let ix = Instruction::new_with_bytes(program_id, &[], vec![]);
    let msg = Message::new(&[ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();

    // Save and restore
    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let mut restored = load_from_file(&path).unwrap();

    // Execute the same program on restored instance
    let user2 = Keypair::new();
    restored.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();
    let ix2 = Instruction::new_with_bytes(program_id, &[], vec![]);
    let msg2 = Message::new(&[ix2], Some(&user2.pubkey()));
    let tx2 = Transaction::new(&[&user2], msg2, restored.latest_blockhash());
    restored.send_transaction(tx2).unwrap();
}

#[test]
fn custom_compute_budget_round_trip() {
    let mut budget = ComputeBudget::new_with_defaults(false, false);
    budget.compute_unit_limit = 500_000;
    budget.max_call_depth = 128;
    budget.stack_frame_size = 8192;
    budget.heap_size = 64 * 1024;

    let svm = LiteSVM::new()
        .with_compute_budget(budget)
        .with_builtins()
        .with_sysvars();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    let restored_budget = restored.get_compute_budget().unwrap();
    assert_eq!(restored_budget.compute_unit_limit, 500_000);
    assert_eq!(restored_budget.max_call_depth, 128);
    assert_eq!(restored_budget.stack_frame_size, 8192);
    assert_eq!(restored_budget.heap_size, 64 * 1024);
}

#[test]
fn fee_structure_round_trip() {
    use solana_fee_structure::{FeeBin, FeeStructure};

    let fee_structure = FeeStructure {
        lamports_per_signature: 10_000,
        lamports_per_write_lock: 500,
        compute_fee_bins: vec![
            FeeBin {
                limit: 500_000,
                fee: 0,
            },
            FeeBin {
                limit: 1_400_000,
                fee: 100,
            },
        ],
    };

    let mut svm = LiteSVM::new().with_builtins().with_sysvars();
    svm.set_fee_structure(fee_structure.clone());

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    let restored_fs = restored.get_fee_structure();
    assert_eq!(restored_fs.lamports_per_signature, 10_000);
    assert_eq!(restored_fs.lamports_per_write_lock, 500);
    assert_eq!(restored_fs.compute_fee_bins.len(), 2);
    assert_eq!(restored_fs.compute_fee_bins[0].limit, 500_000);
    assert_eq!(restored_fs.compute_fee_bins[0].fee, 0);
    assert_eq!(restored_fs.compute_fee_bins[1].limit, 1_400_000);
    assert_eq!(restored_fs.compute_fee_bins[1].fee, 100);
}

#[test]
fn history_capacity_round_trip() {
    let svm = LiteSVM::new()
        .with_builtins()
        .with_sysvars()
        .with_transaction_history(500);

    let original_cap = svm.get_history_capacity();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    // Capacity should be preserved exactly (IndexMap may round up from the
    // requested value, but the persisted value is the actual allocation).
    assert_eq!(restored.get_history_capacity(), original_cap);
}

#[test]
fn bpf_program_executes_with_custom_compute_budget() {
    let mut budget = ComputeBudget::new_with_defaults(false, false);
    budget.compute_unit_limit = 2_000_000;

    let mut svm = LiteSVM::new()
        .with_compute_budget(budget)
        .with_builtins()
        .with_sysvars();

    let program_bytes = include_bytes!("../../node-litesvm/program_bytes/spl_example_logging.so");
    let program_id = Address::new_unique();
    svm.add_program(program_id, program_bytes).unwrap();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let mut restored = load_from_file(&path).unwrap();

    // Verify the budget survived
    assert_eq!(
        restored.get_compute_budget().unwrap().compute_unit_limit,
        2_000_000
    );

    // Verify the program still executes
    let user = Keypair::new();
    restored.airdrop(&user.pubkey(), 1_000_000_000).unwrap();
    let ix = Instruction::new_with_bytes(program_id, &[], vec![]);
    let msg = Message::new(&[ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, restored.latest_blockhash());
    restored.send_transaction(tx).unwrap();
}

#[test]
fn load_nonexistent_file_returns_error() {
    let result = load_from_file("/tmp/litesvm_does_not_exist_12345.bin");
    assert!(matches!(result, Err(PersistenceError::Io(_))));
}

#[test]
fn load_corrupted_data_returns_error() {
    // Too short to even contain a checksum.
    let result = from_bytes(&[0xFF, 0xFE, 0xFD]);
    assert!(matches!(result, Err(PersistenceError::Serialize(_))));
}

#[test]
fn checksum_detects_corruption() {
    let svm = LiteSVM::new().with_builtins().with_sysvars();
    let mut bytes = to_bytes(&svm).unwrap();
    // Flip a byte in the payload (not the checksum trailer).
    bytes[0] ^= 0xFF;
    let result = from_bytes(&bytes);
    assert!(
        matches!(result, Err(PersistenceError::ChecksumMismatch { .. })),
        "expected ChecksumMismatch"
    );
}

#[test]
fn corrupted_file_returns_checksum_error() {
    let svm = LiteSVM::new().with_builtins().with_sysvars();
    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();

    // Corrupt the file on disk.
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[10] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();

    let result = load_from_file(&path);
    assert!(
        matches!(result, Err(PersistenceError::ChecksumMismatch { .. })),
        "expected ChecksumMismatch"
    );
}

#[test]
fn double_round_trip() {
    // Save -> restore -> modify -> save -> restore
    let mut svm = LiteSVM::new()
        .with_builtins()
        .with_sysvars()
        .with_sigverify(false)
        .with_blockhash_check(false);

    let addr = Address::new_unique();
    let account = solana_account::Account::new(1_000_000, 64, &Address::default());
    svm.set_account(addr, account).unwrap();

    // First round trip.
    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let mut restored1 = load_from_file(&path).unwrap();

    // Modify the restored instance.
    let addr2 = Address::new_unique();
    let account2 = solana_account::Account::new(2_000_000, 32, &Address::default());
    restored1.set_account(addr2, account2).unwrap();
    restored1.expire_blockhash();
    let hash_after = restored1.latest_blockhash();

    // Second round trip.
    let dir2 = temp_dir();
    let path2 = dir2.path().join("snapshot.bin");
    save_to_file(&restored1, &path2).unwrap();
    let restored2 = load_from_file(&path2).unwrap();

    // Verify both accounts survive.
    assert_eq!(restored2.get_account(&addr).unwrap().lamports, 1_000_000);
    assert_eq!(restored2.get_account(&addr2).unwrap().lamports, 2_000_000);
    assert_eq!(restored2.latest_blockhash(), hash_after);
}

#[test]
fn full_default_round_trip() {
    // LiteSVM::new() includes builtins, sysvars, default programs, lamports,
    // sigverify=true, blockhash_check=true — the full default setup.
    let svm = LiteSVM::new();

    let dir = temp_dir();
    let path = dir.path().join("snapshot.bin");
    save_to_file(&svm, &path).unwrap();
    let mut restored = load_from_file(&path).unwrap();

    assert_eq!(restored.get_sigverify(), true);
    assert_eq!(restored.get_blockhash_check(), true);

    // Verify the restored instance is functional: airdrop + system transfer.
    let from_kp = Keypair::new();
    let to = Address::new_unique();
    restored.airdrop(&from_kp.pubkey(), 10_000_000_000).unwrap();

    let ix = solana_system_interface::instruction::transfer(&from_kp.pubkey(), &to, 1_000_000_000);
    let msg = Message::new(&[ix], Some(&from_kp.pubkey()));
    let tx = Transaction::new(&[&from_kp], msg, restored.latest_blockhash());
    restored.send_transaction(tx).unwrap();

    assert_eq!(restored.get_balance(&to).unwrap(), 1_000_000_000);
}
