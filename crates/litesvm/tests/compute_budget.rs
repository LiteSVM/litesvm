use {
    litesvm::LiteSVM, solana_compute_budget::compute_budget::ComputeBudget,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::error::InstructionError, solana_keypair::Keypair, solana_message::Message,
    solana_pubkey::Pubkey, solana_signer::Signer, solana_system_interface::instruction::transfer,
    solana_transaction::Transaction, solana_transaction_error::TransactionError,
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
