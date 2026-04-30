//! Tests for BPF Upgradeable Loader program auto-loading functionality.
//!
//! This module tests the fix for GitHub issue #240:
//! https://github.com/LiteSVM/litesvm/issues/240
//!
//! The issue: When programs are deployed via BPF Loader Upgradeable, only the
//! ProgramData account gets written to the ExecutionRecord, not the Program
//! account. This causes the program to not be loaded into the cache, making
//! subsequent transactions fail.

use {
    agave_feature_set::FeatureSet,
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_loader_v3_interface::{
        instruction as loader_v3_instruction, state::UpgradeableLoaderState,
    },
    solana_message::{Message, VersionedMessage},
    solana_native_token::LAMPORTS_PER_SOL,
    solana_sdk_ids::bpf_loader_upgradeable,
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
};

/// Helper to read a test program that we can use for testing.
fn read_test_program() -> Vec<u8> {
    include_bytes!("../test_programs/DF1ow3DqMj3HvTj8i8J9yM2hE9hCrLLXpdbaKZu4ZPnz.so").to_vec()
}

/// Creates a Program account (the executable pointer) for BPF Loader Upgradeable.
fn create_program_account(programdata_address: &Address, executable: bool) -> Account {
    let state = UpgradeableLoaderState::Program {
        programdata_address: *programdata_address,
    };
    let data = bincode::serialize(&state).unwrap();
    Account {
        lamports: 1_000_000,
        data,
        owner: bpf_loader_upgradeable::id(),
        executable,
        rent_epoch: 0,
    }
}

/// Creates a ProgramData account containing the actual program bytes.
fn create_programdata_account(program_bytes: &[u8], upgrade_authority: Option<Address>) -> Account {
    let state = UpgradeableLoaderState::ProgramData {
        slot: 0,
        upgrade_authority_address: upgrade_authority,
    };
    let mut data = bincode::serialize(&state).unwrap();
    data.resize(UpgradeableLoaderState::size_of_programdata_metadata(), 0);
    data.extend_from_slice(program_bytes);

    Account {
        lamports: 10_000_000,
        data,
        owner: bpf_loader_upgradeable::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn new_v3_deploy_svm() -> LiteSVM {
    LiteSVM::default()
        .with_feature_set(FeatureSet::all_enabled())
        .with_builtins()
        .with_lamports(1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL))
        .with_sysvars()
}

fn deploy_upgradeable_program(svm: &mut LiteSVM, payer: &Keypair, program_bytes: &[u8]) -> Address {
    const CHUNK_SIZE: usize = 512;

    let buffer = Keypair::new();
    let program = Keypair::new();
    let payer_address = payer.pubkey();

    let buffer_len = UpgradeableLoaderState::size_of_buffer(program_bytes.len());
    let buffer_lamports = svm.minimum_balance_for_rent_exemption(buffer_len);
    let create_buffer_tx = Transaction::new_signed_with_payer(
        &loader_v3_instruction::create_buffer(
            &payer_address,
            &buffer.pubkey(),
            &payer_address,
            buffer_lamports,
            program_bytes.len(),
        )
        .unwrap(),
        Some(&payer_address),
        &[payer, &buffer],
        svm.latest_blockhash(),
    );
    svm.send_transaction(create_buffer_tx).unwrap();

    for (chunk_idx, chunk) in program_bytes.chunks(CHUNK_SIZE).enumerate() {
        let write_tx = Transaction::new_signed_with_payer(
            &[loader_v3_instruction::write(
                &buffer.pubkey(),
                &payer_address,
                (chunk_idx * CHUNK_SIZE) as u32,
                chunk.to_vec(),
            )],
            Some(&payer_address),
            &[payer],
            svm.latest_blockhash(),
        );
        svm.send_transaction(write_tx).unwrap();
    }

    let deploy_lamports = svm.minimum_balance_for_rent_exemption(program_bytes.len());
    #[allow(deprecated)]
    let deploy_tx = Transaction::new_signed_with_payer(
        &loader_v3_instruction::deploy_with_max_program_len(
            &payer_address,
            &program.pubkey(),
            &buffer.pubkey(),
            &payer_address,
            deploy_lamports,
            program_bytes.len() * 2,
        )
        .unwrap(),
        Some(&payer_address),
        &[payer, &program],
        svm.latest_blockhash(),
    );
    svm.send_transaction(deploy_tx).unwrap();

    program.pubkey()
}

/// Test that programs can be invoked after being set up as upgradeable loader accounts.
#[test]
fn test_program_with_upgradeable_loader_can_be_invoked() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let program_id = Address::new_unique();
    let programdata_address = Address::new_unique();
    let program_bytes = read_test_program();

    // Create the ProgramData account first
    let programdata_account = create_programdata_account(&program_bytes, None);
    svm.set_account(programdata_address, programdata_account)
        .unwrap();

    // Create the Program account with executable=TRUE
    let program_account = create_program_account(&programdata_address, true);
    svm.set_account(program_id, program_account).unwrap();

    // Verify the program account is set up correctly
    let stored_program = svm.get_account(&program_id).unwrap();
    assert!(stored_program.executable);
    assert_eq!(stored_program.owner, bpf_loader_upgradeable::id());

    // Now try to invoke the program - the auto-load logic should load it into the cache
    let ix = Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let result = svm.send_transaction(tx);

    // The program should be found and loaded (even if execution fails for other reasons)
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(
                e.err != solana_transaction_error::TransactionError::InvalidProgramForExecution,
                "Program should have been found and loaded"
            );
            assert!(
                e.err != solana_transaction_error::TransactionError::ProgramAccountNotFound,
                "Program account should exist"
            );
        }
    }
}

/// Test that programs not in cache are auto-loaded during transaction execution.
#[test]
fn test_program_auto_load_on_transaction() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let program_id = Address::new_unique();
    let programdata_address = Address::new_unique();
    let program_bytes = read_test_program();

    // Create the ProgramData account first
    let programdata_account = create_programdata_account(&program_bytes, None);
    svm.set_account(programdata_address, programdata_account)
        .unwrap();

    // Create the Program account with executable=true
    let program_account = create_program_account(&programdata_address, true);
    svm.set_account(program_id, program_account).unwrap();

    // Now try to invoke the program
    let ix = Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let result = svm.send_transaction(tx);

    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(
                e.err != solana_transaction_error::TransactionError::InvalidProgramForExecution,
                "Program should have been auto-loaded"
            );
            assert!(
                e.err != solana_transaction_error::TransactionError::ProgramAccountNotFound,
                "Program should have been auto-loaded"
            );
        }
    }
}

/// Test the load_existing_programs() public API.
#[test]
fn test_load_existing_programs_api() {
    let mut svm = LiteSVM::new();

    let program_id = Address::new_unique();
    let programdata_address = Address::new_unique();
    let program_bytes = read_test_program();

    // Create accounts
    let programdata_account = create_programdata_account(&program_bytes, None);
    svm.set_account(programdata_address, programdata_account)
        .unwrap();

    let program_account = create_program_account(&programdata_address, true);
    svm.set_account(program_id, program_account).unwrap();

    // Call load_existing_programs
    svm.load_existing_programs()
        .expect("load_existing_programs should succeed");

    // Verify by attempting to call the program
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let result = svm.send_transaction(tx);

    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(
                e.err != solana_transaction_error::TransactionError::InvalidProgramForExecution,
                "Program should be loaded after load_existing_programs()"
            );
        }
    }
}

/// Test that BPF loader accounts are synced even when not in the writable set.
#[test]
fn test_bpf_loader_accounts_synced() {
    let mut svm = new_v3_deploy_svm();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    let program_id = deploy_upgradeable_program(&mut svm, &payer, &read_test_program());

    let stored = svm.get_account(&program_id).unwrap();
    assert!(stored.executable);
    assert_eq!(stored.owner, bpf_loader_upgradeable::id());

    let ix = Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    };
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let result = svm.send_transaction(tx);

    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(
                e.err != solana_transaction_error::TransactionError::InvalidProgramForExecution,
                "Program account should have been synced and loaded after deploy"
            );
            assert!(
                e.err != solana_transaction_error::TransactionError::ProgramAccountNotFound,
                "Program account should have been synced after deploy"
            );
        }
    }
}
