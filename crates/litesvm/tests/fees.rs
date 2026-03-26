use {
    jupnet_sdk::{
        instruction::{Instruction, InstructionError},
        message::Message,
        pubkey::Pubkey,
        rent::Rent,
        signer::{keypair::Keypair, Signer},
        system_instruction::transfer,
        transaction::{Transaction, TransactionError},
    },
    litesvm::LiteSVM,
    std::path::PathBuf,
};

#[test_log::test]
fn test_fee_payer_insufficient_funds_for_rent() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

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
    let signature = tx.signatures[0].clone();
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
    let program_id = Pubkey::from_str_const("HvrRMSshMx3itvsyWDnWg2E3cy5h57iMaR7oVxSZJDSA");
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
    let signature = tx.signatures[0].clone();
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
