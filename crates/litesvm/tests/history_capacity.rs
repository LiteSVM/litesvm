use {
    litesvm::LiteSVM, solana_address::Address, solana_keypair::Keypair, solana_signer::Signer,
    solana_system_interface::instruction::transfer, solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

fn fixture(capacity: usize) -> (LiteSVM, Keypair, Address) {
    let mut svm = LiteSVM::new().with_transaction_history(capacity);
    let payer = Keypair::new();
    let recipient = Address::new_unique();
    svm.airdrop(&payer.pubkey(), 10_000_000).unwrap();
    svm.airdrop(&recipient, 1_000_000).unwrap();
    (svm, payer, recipient)
}

fn transaction(svm: &LiteSVM, payer: &Keypair, recipient: &Address, amount: u64) -> Transaction {
    Transaction::new_signed_with_payer(
        &[transfer(&payer.pubkey(), recipient, amount)],
        Some(&payer.pubkey()),
        &[payer],
        svm.latest_blockhash(),
    )
}

#[test]
fn configured_capacity_retains_exactly_the_latest_transactions() {
    for capacity in [1, 2, 4, 32, 33] {
        let (mut svm, payer, recipient) = fixture(capacity);
        let mut signatures = Vec::new();
        for amount in 1..=capacity as u64 + 2 {
            let tx = transaction(&svm, &payer, &recipient, amount);
            let meta = svm.send_transaction(tx).unwrap();
            signatures.push(meta.signature);
        }
        for (index, signature) in signatures.iter().enumerate() {
            assert_eq!(
                svm.get_transaction(signature).is_some(),
                index >= 2,
                "capacity {capacity}, transaction {index}"
            );
        }
        let tx = transaction(&svm, &payer, &recipient, capacity as u64 + 2);
        let balance = svm.get_balance(&payer.pubkey());
        assert_eq!(
            svm.send_transaction(tx).unwrap_err().err,
            TransactionError::AlreadyProcessed
        );
        assert_eq!(svm.get_balance(&payer.pubkey()), balance);
    }
}

#[test]
fn increasing_capacity_with_few_entries_uses_the_requested_limit() {
    let (mut svm, payer, recipient) = fixture(32);
    svm = svm.with_transaction_history(33);
    let mut signatures = Vec::new();
    for amount in 1..=34 {
        let tx = transaction(&svm, &payer, &recipient, amount);
        signatures.push(svm.send_transaction(tx).unwrap().signature);
    }
    assert!(svm.get_transaction(&signatures[0]).is_none());
    for signature in &signatures[1..] {
        assert!(svm.get_transaction(signature).is_some());
    }
}

#[test]
fn setting_capacity_after_partial_use_keeps_the_configured_limit() {
    let (mut svm, payer, recipient) = fixture(32);
    svm = svm.with_transaction_history(8);
    svm = svm.with_transaction_history(8);
    let mut signatures = Vec::new();
    for amount in 1..=9 {
        let tx = transaction(&svm, &payer, &recipient, amount);
        signatures.push(svm.send_transaction(tx).unwrap().signature);
    }
    assert!(svm.get_transaction(&signatures[0]).is_none());
    for signature in &signatures[1..] {
        assert!(svm.get_transaction(signature).is_some());
    }
}

#[test]
fn history_can_be_disabled_and_reenabled_with_capacity_one() {
    let (mut svm, payer, recipient) = fixture(32);
    let tx = transaction(&svm, &payer, &recipient, 1);
    let signature = tx.signatures[0];
    svm.send_transaction(tx.clone()).unwrap();
    svm = svm.with_transaction_history(0);
    assert!(svm.get_transaction(&signature).is_none());
    svm.send_transaction(tx.clone()).unwrap();
    assert!(svm.get_transaction(&signature).is_none());
    svm = svm.with_transaction_history(1);
    svm.send_transaction(tx).unwrap();
    assert!(svm.get_transaction(&signature).is_some());
    let next = transaction(&svm, &payer, &recipient, 2);
    let next_signature = svm.send_transaction(next).unwrap().signature;
    assert!(svm.get_transaction(&signature).is_none());
    assert!(svm.get_transaction(&next_signature).is_some());
}

#[test]
fn failed_transactions_use_the_same_retention_limit() {
    let (mut svm, payer, recipient) = fixture(1);
    let tx = transaction(&svm, &payer, &recipient, u64::MAX);
    let signature = tx.signatures[0];
    let failed = svm.send_transaction(tx.clone()).unwrap_err();
    assert_eq!(svm.get_transaction(&signature), Some(&Err(failed)));
    assert_eq!(
        svm.send_transaction(tx).unwrap_err().err,
        TransactionError::AlreadyProcessed
    );
    let next = transaction(&svm, &payer, &recipient, 1);
    svm.send_transaction(next).unwrap();
    assert!(svm.get_transaction(&signature).is_none());
}
