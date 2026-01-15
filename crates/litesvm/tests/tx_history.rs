use {
    litesvm::LiteSVM, solana_address::Address, solana_keypair::Keypair, solana_signer::Signer,
    solana_system_interface::instruction::transfer, solana_transaction::Transaction,
};

#[test]
fn test_tx_history_base_case() {
    // Create a key for our test transactions
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    // Create the VM and airdrop funds
    let mut svm = LiteSVM::new()
        .with_sigverify(false)
        .with_blockhash_check(false)
        .with_transaction_history(0);
    svm.airdrop(&from, 10_000_000).unwrap();

    // Try to create and send two identical transactions
    let instruction = transfer(&from, &to, 100);

    // First transaction - should succeed
    // Note: unsigned transactions use default (the same) signatures
    let tx1 = Transaction::new_with_payer(std::slice::from_ref(&instruction), Some(&from));
    let result1 = svm.send_transaction(tx1);
    assert!(result1.is_ok(), "First transaction should succeed");

    // Second duplicate transaction - should succeed
    let tx2 = Transaction::new_with_payer(&[instruction], Some(&from));
    let result2 = svm.send_transaction(tx2);

    assert!(result2.is_ok(), "Second transaction should succeed");
}

/// Setting the transaction history to 0 at any time should disable transaction history
#[test]
fn test_tx_history_disable_later() {
    // Create a key for our test transactions
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    // Create the VM and airdrop funds
    let mut svm = LiteSVM::new()
        .with_sigverify(false)
        .with_blockhash_check(false);
    svm.airdrop(&from, 10_000_000).unwrap();

    // Try to create and send two identical transactions
    let instruction = transfer(&from, &to, 100);

    // Note: unsigned transactions use default (the same) signatures as the airdrop
    let tx1 = Transaction::new_with_payer(std::slice::from_ref(&instruction), Some(&from));
    let result1 = svm.send_transaction(tx1);
    assert!(result1.is_ok(), "First transaction should succeed");

    let mut svm = svm.with_transaction_history(0);

    // Second duplicate transaction - should succeed
    let tx2 = Transaction::new_with_payer(&[instruction], Some(&from));
    let result2 = svm.send_transaction(tx2.clone());

    assert!(result2.is_ok(), "Second transaction should succeed");

    // Get should not panic
    let result = svm.get_transaction(&tx2.signatures[0]);
    assert!(result.is_none(), "Transaction should not be in history");
}
