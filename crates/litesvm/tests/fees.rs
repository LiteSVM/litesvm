use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::{address, Address},
    solana_instruction::{error::InstructionError, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    std::path::PathBuf,
};

#[test_log::test]
fn test_fee_payer_insufficient_funds_for_rent() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();

    svm.airdrop(&from, svm.get_sysvar::<Rent>().minimum_balance(0))
        .unwrap();
    svm.airdrop(&to, svm.get_sysvar::<Rent>().minimum_balance(0))
        .unwrap();
    let instruction = transfer(&from, &to, 1);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let signature = tx.signatures[0];
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InsufficientFundsForRent { account_index: 0 }
    );
    assert!(svm.get_transaction(&signature).is_none());
}

#[test_log::test]
fn test_fees_failed_transaction() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    let program_id = address!("HvrRMSshMx3itvsyWDnWg2E3cy5h57iMaR7oVxSZJDSA");
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/failure.so");
    svm.add_program_from_file(program_id, &so_path).unwrap();
    let initial_balance = 1_000_000_000;
    svm.airdrop(&from, initial_balance).unwrap();
    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    };
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let signature = tx.signatures[0];
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::Custom(0))
    );
    let balance_after = svm.get_balance(&from).unwrap();
    let expected_fee = 5000;
    assert_eq!(initial_balance - balance_after, expected_fee);
    assert!(svm.get_transaction(&signature).unwrap().is_err());
}

#[test_log::test]
fn test_fee_that_drains_payer_of_failed_transaction_removes_it() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    // Exactly the fee, so a failed transaction leaves the payer with zero lamports.
    svm.set_account(
        from,
        Account {
            lamports: 5000,
            ..Default::default()
        },
    )
    .unwrap();
    let instruction = transfer(&from, &Address::new_unique(), 1);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::Custom(1))
    );
    // A zero-lamport account does not exist, as when a successful transaction drains it.
    assert_eq!(svm.get_balance(&from), None);
    assert!(svm.get_account(&from).is_none());
}
