use {
    jupnet_sdk::{
        pubkey::Pubkey,
        signer::{keypair::Keypair, Signer},
        system_instruction::transfer,
        transaction::Transaction,
    },
    litesvm::LiteSVM,
};

#[test]
fn test_tx_history_base_case() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new()
        .with_sigverify(false)
        .with_blockhash_check(false)
        .with_transaction_history(0);
    svm.airdrop(&from, 10_000_000).unwrap();
    svm.airdrop(&to, 10_000_000).unwrap();

    let instruction = transfer(&from, &to, 100);

    let tx1 = Transaction::new_with_payer(std::slice::from_ref(&instruction), Some(&from));
    let result1 = svm.send_transaction(tx1);
    assert!(result1.is_ok(), "First transaction should succeed");

    let tx2 = Transaction::new_with_payer(&[instruction], Some(&from));
    let result2 = svm.send_transaction(tx2);

    assert!(result2.is_ok(), "Second transaction should succeed");
}

/// Setting the transaction history to 0 at any time should disable transaction history
#[test]
fn test_tx_history_disable_later() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new()
        .with_sigverify(false)
        .with_blockhash_check(false);
    svm.airdrop(&from, 10_000_000).unwrap();
    svm.airdrop(&to, 10_000_000).unwrap();

    let instruction = transfer(&from, &to, 100);

    let tx1 = Transaction::new_with_payer(std::slice::from_ref(&instruction), Some(&from));
    let result1 = svm.send_transaction(tx1);
    assert!(result1.is_ok(), "First transaction should succeed");

    let mut svm = svm.with_transaction_history(0);

    let tx2 = Transaction::new_with_payer(&[instruction], Some(&from));
    let result2 = svm.send_transaction(tx2.clone());

    assert!(result2.is_ok(), "Second transaction should succeed");

    let result = svm.get_transaction(&tx2.signatures[0]);
    assert!(result.is_none(), "Transaction should not be in history");
}
