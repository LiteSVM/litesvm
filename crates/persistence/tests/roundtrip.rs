use {
    litesvm::LiteSVM,
    litesvm_persistence::{from_bytes, load_from_file, save_to_file, to_bytes, PersistenceError},
    solana_account::Account,
    solana_address::Address,
    solana_clock::Clock,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_native_token::LAMPORTS_PER_SOL,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    std::path::PathBuf,
};

/// Helper: create a seeded LiteSVM with builtins, sysvars, and an airdropped account.
fn seeded_svm() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();
    (svm, kp)
}

#[test]
fn basic_account_round_trip() {
    let (svm, kp) = seeded_svm();
    let balance_before = svm.get_balance(&kp.pubkey()).unwrap();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    assert_eq!(restored.get_balance(&kp.pubkey()).unwrap(), balance_before);
}

#[test]
fn multiple_accounts_round_trip() {
    let mut svm = LiteSVM::new();
    let mut addresses = Vec::new();
    for i in 0..10 {
        let addr = Address::new_unique();
        svm.airdrop(&addr, (i + 1) * LAMPORTS_PER_SOL).unwrap();
        addresses.push(addr);
    }

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    for (i, addr) in addresses.iter().enumerate() {
        assert_eq!(
            restored.get_balance(addr).unwrap(),
            (i as u64 + 1) * LAMPORTS_PER_SOL
        );
    }
}

#[test]
fn sysvar_round_trip() {
    let mut svm = LiteSVM::new();
    svm.warp_to_slot(42);

    let clock_before: Clock = svm.get_sysvar();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    let clock_after: Clock = restored.get_sysvar();
    assert_eq!(clock_before.slot, clock_after.slot);
    assert_eq!(42, clock_after.slot);
}

#[test]
fn config_round_trip() {
    let svm = LiteSVM::new()
        .with_sigverify(true)
        .with_blockhash_check(true)
        .with_log_bytes_limit(Some(5000));

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    assert_eq!(restored.get_sigverify(), true);
    assert_eq!(restored.get_blockhash_check(), true);
    assert_eq!(restored.get_log_bytes_limit(), Some(5000));
}

#[test]
fn blockhash_round_trip() {
    let mut svm = LiteSVM::new();
    svm.expire_blockhash();
    let bh = svm.latest_blockhash();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    assert_eq!(restored.latest_blockhash(), bh);
}

#[test]
fn airdrop_keypair_round_trip() {
    let svm = LiteSVM::new();
    let airdrop_pk = svm.airdrop_pubkey();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    assert_eq!(restored.airdrop_pubkey(), airdrop_pk);
}

#[test]
fn transaction_history_round_trip() {
    let (mut svm, kp) = seeded_svm();
    let to = Address::new_unique();
    svm.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    let ix = transfer(&kp.pubkey(), &to, 1_000);
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        svm.latest_blockhash(),
    );
    let result = svm.send_transaction(tx).unwrap();
    let sig = result.signature;

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    assert!(restored.get_transaction(&sig).is_some());
}

#[test]
fn bytes_round_trip() {
    let (svm, kp) = seeded_svm();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    assert_eq!(
        restored.get_balance(&kp.pubkey()).unwrap(),
        svm.get_balance(&kp.pubkey()).unwrap()
    );
}

#[test]
fn file_round_trip() {
    let (svm, kp) = seeded_svm();
    let balance = svm.get_balance(&kp.pubkey()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.bin");

    save_to_file(&svm, &path).unwrap();
    let restored = load_from_file(&path).unwrap();

    assert_eq!(restored.get_balance(&kp.pubkey()).unwrap(), balance);
}

#[test]
fn airdrop_works_after_restore() {
    let (svm, _kp) = seeded_svm();
    let bytes = to_bytes(&svm).unwrap();
    let mut restored = from_bytes(&bytes).unwrap();

    let new_addr = Address::new_unique();
    restored.airdrop(&new_addr, 5 * LAMPORTS_PER_SOL).unwrap();
    assert_eq!(
        restored.get_balance(&new_addr).unwrap(),
        5 * LAMPORTS_PER_SOL
    );
}

#[test]
fn send_transaction_after_restore() {
    let (svm, kp) = seeded_svm();
    let bytes = to_bytes(&svm).unwrap();
    let mut restored = from_bytes(&bytes).unwrap();

    // Need a fresh blockhash for the restored instance
    restored.expire_blockhash();

    let to = Address::new_unique();
    restored.airdrop(&to, LAMPORTS_PER_SOL).unwrap();

    let ix = transfer(&kp.pubkey(), &to, 500_000);
    let tx = Transaction::new(
        &[&kp],
        Message::new(&[ix], Some(&kp.pubkey())),
        restored.latest_blockhash(),
    );
    let result = restored.send_transaction(tx);
    assert!(result.is_ok());
}

#[test]
fn load_nonexistent_file() {
    let result = load_from_file("/tmp/nonexistent_litesvm_snapshot_abc123.bin");
    assert!(matches!(result, Err(PersistenceError::Io(_))));
}

#[test]
fn load_corrupted_data() {
    let result = from_bytes(&[1, 0, 0, 0, 0xff, 0xff]); // version 1 + garbage
    assert!(matches!(result, Err(PersistenceError::Serialize(_))));
}

#[test]
fn version_check() {
    let result = from_bytes(&[255, 0, 0, 0]); // invalid version
    assert!(matches!(result, Err(PersistenceError::UnsupportedVersion(255))));
}

#[test]
fn bpf_program_round_trip() {
    // Load the counter program
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("../litesvm/test_programs/target/deploy/counter.so");
    let program_bytes = std::fs::read(&so_path).expect("counter.so not found — run `cd crates/litesvm/test_programs && cargo build-sbf`");

    let program_id =
        Address::from_str_const("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    let counter_address =
        Address::from_str_const("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");

    // Set up SVM with a deployed BPF program and counter account
    let mut svm = LiteSVM::new();
    svm.add_program(program_id, &program_bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();
    svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    )
    .unwrap();

    // Increment counter once before snapshot
    let ix = Instruction {
        program_id,
        accounts: vec![AccountMeta::new(counter_address, false)],
        data: vec![0, 0],
    };
    let tx = Transaction::new(
        &[&payer],
        Message::new(&[ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        1u32.to_le_bytes().to_vec()
    );

    // Save and restore
    let bytes = to_bytes(&svm).unwrap();
    let mut restored = from_bytes(&bytes).unwrap();

    // Verify counter value preserved
    assert_eq!(
        restored.get_account(&counter_address).unwrap().data,
        1u32.to_le_bytes().to_vec()
    );

    // Increment counter on restored instance — proves program cache was rebuilt
    restored.expire_blockhash();
    let ix = Instruction {
        program_id,
        accounts: vec![AccountMeta::new(counter_address, false)],
        data: vec![0, 1],
    };
    let tx = Transaction::new(
        &[&payer],
        Message::new(&[ix], Some(&payer.pubkey())),
        restored.latest_blockhash(),
    );
    restored.send_transaction(tx).unwrap();
    assert_eq!(
        restored.get_account(&counter_address).unwrap().data,
        2u32.to_le_bytes().to_vec()
    );
}

#[test]
fn account_with_data_round_trip() {
    let mut svm = LiteSVM::new();
    let addr = Address::new_unique();
    let owner = Address::new_unique();
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
    svm.set_account(
        addr,
        Account {
            lamports: 1_000_000,
            data: data.clone(),
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let bytes = to_bytes(&svm).unwrap();
    let restored = from_bytes(&bytes).unwrap();

    let account = restored.get_account(&addr).unwrap();
    assert_eq!(account.data, data);
    assert_eq!(account.owner, owner);
    assert_eq!(account.lamports, 1_000_000);
}
