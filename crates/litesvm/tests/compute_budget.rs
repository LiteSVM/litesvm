use litesvm::LiteSVM;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program::{
    instruction::InstructionError, message::Message, pubkey::Pubkey, system_instruction::transfer,
};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction, TransactionError},
};

#[test_log::test]
fn test_set_compute_budget() {
    // see that the tx fails if we set a tiny limit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();
    let tx_fee = 5000;
    svm.airdrop(&from, tx_fee + 100).unwrap();
    // need to set the low compute budget after the airdrop tx
    svm = svm.with_compute_budget(ComputeBudget {
        compute_unit_limit: 10,
        ..Default::default()
    });
    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::ComputationalBudgetExceeded)
    );
}

#[test]
fn test_set_compute_unit_limit() {
    // see that the tx fails if we set a tiny limit
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();
    let tx_fee = 5000;

    svm.airdrop(&from, tx_fee + 100).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(10),
                instruction,
            ],
            Some(&from),
        ),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(
        tx_res.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::ComputationalBudgetExceeded)
    );
}

#[test_log::test]
fn test_compute_tracker() {
    let mut svm = LiteSVM::new();
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    // Airdrop some funds
    svm.airdrop(&from, 1_000_000).unwrap();

    // Initial state should be zero
    let stats = svm.get_compute_stats();
    assert_eq!(stats.total_compute_units(), 150);
    assert_eq!(stats.transaction_count(), 1);
    assert_eq!(stats.min_compute_units(), 150);
    assert_eq!(stats.max_compute_units(), 150);
    assert_eq!(stats.average_compute_units(), 150.0);

    // Send a transfer transaction
    let instruction = transfer(&from, &to, 100);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Check compute units were tracked
    let stats = svm.get_compute_stats();
    assert!(stats.total_compute_units() > 0);
    assert_eq!(stats.transaction_count(), 2);
    assert!(stats.min_compute_units() > 0);
    assert!(stats.max_compute_units() > 0);
    assert!(stats.average_compute_units() > 0.0);

    // Send another transaction
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[transfer(&from, &to, 50)], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Check stats were updated
    let stats = svm.get_compute_stats();
    assert_eq!(stats.transaction_count(), 3);
    
    // Test reset
    svm.reset_compute_tracker();
    let stats = svm.get_compute_stats();
    assert_eq!(stats.total_compute_units(), 0);
    assert_eq!(stats.transaction_count(), 0);
    assert_eq!(stats.min_compute_units(), 0);
    assert_eq!(stats.max_compute_units(), 0);
    assert_eq!(stats.average_compute_units(), 0.0);
}
