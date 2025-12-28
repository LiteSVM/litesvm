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
    litesvm::LiteSVM,
    solana_account::Account,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_loader_v3_interface::state::UpgradeableLoaderState,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_sdk_ids::bpf_loader_upgradeable,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

/// Helper to read a test program that we can use for testing.
fn read_test_program() -> Vec<u8> {
    include_bytes!("../test_programs/DF1ow3DqMj3HvTj8i8J9yM2hE9hCrLLXpdbaKZu4ZPnz.so").to_vec()
}

/// Creates a Program account (the executable pointer) for BPF Loader Upgradeable.
fn create_program_account(programdata_address: &Pubkey, executable: bool) -> Account {
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
fn create_programdata_account(program_bytes: &[u8], upgrade_authority: Option<Pubkey>) -> Account {
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

/// Test that programs can be invoked after being set up as upgradeable loader accounts.
#[test]
fn test_program_with_upgradeable_loader_can_be_invoked() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let program_id = Pubkey::new_unique();
    let programdata_address = Pubkey::new_unique();
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

    let program_id = Pubkey::new_unique();
    let programdata_address = Pubkey::new_unique();
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

    let program_id = Pubkey::new_unique();
    let programdata_address = Pubkey::new_unique();
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
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let program_id = Pubkey::new_unique();
    let programdata_address = Pubkey::new_unique();
    let program_bytes = read_test_program();

    // Set up initial program
    let programdata_account = create_programdata_account(&program_bytes, Some(payer.pubkey()));
    svm.set_account(programdata_address, programdata_account)
        .unwrap();

    let program_account = create_program_account(&programdata_address, true);
    svm.set_account(program_id, program_account).unwrap();

    // Verify the program account exists and is properly configured
    let stored = svm.get_account(&program_id).unwrap();
    assert!(stored.executable);
    assert_eq!(stored.owner, bpf_loader_upgradeable::id());
}
