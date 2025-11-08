use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

#[test]
fn test_account_tracking_disabled_by_default() {
    let mut svm = LiteSVM::new();
    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to = Pubkey::new_unique();

    svm.airdrop(&from, 10_000).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_kp],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx).unwrap();

    // Tracking disabled by default, so accessed_accounts should be None
    assert!(result.accessed_accounts.is_none());
}

#[test]
fn test_account_tracking_enabled() {
    let mut svm = LiteSVM::new().with_account_tracking(true);

    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to = Pubkey::new_unique();

    svm.airdrop(&from, 10_000).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_kp],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx).unwrap();

    // Tracking enabled, so we should have accessed accounts
    let accessed = result
        .accessed_accounts
        .expect("Should have accessed accounts");

    // Should have accessed at least the from and to accounts
    assert!(accessed.contains(&from));
    assert!(accessed.contains(&to));

    // Should have accessed some accounts (from, to, system program, etc.)
    assert!(accessed.len() >= 2);
}

#[test]
fn test_missing_account_detection() {
    use solana_instruction::{AccountMeta, Instruction};

    let mut svm = LiteSVM::new().with_account_tracking(true);

    let payer_kp = Keypair::new();
    let payer = payer_kp.pubkey();
    let missing_account = Pubkey::new_unique();

    svm.airdrop(&payer, 10_000_000).unwrap();

    // Create an instruction that references a non-existent program
    let program_id = Pubkey::new_unique();
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(missing_account, false),
        ],
        data: vec![],
    };

    let tx = Transaction::new(
        &[&payer_kp],
        Message::new(&[ix], Some(&payer)),
        svm.latest_blockhash(),
    );

    // Transaction will fail because program doesn't exist
    let failed = svm.send_transaction(tx).unwrap_err();

    // But we can see which accounts were accessed
    let accessed = failed
        .meta
        .accessed_accounts
        .expect("Should have accessed accounts");

    // Find accounts that don't exist
    let missing: Vec<_> = accessed
        .iter()
        .filter(|pk| svm.get_account(pk).is_none())
        .collect();

    // Should include the missing account and/or the missing program
    assert!(!missing.is_empty(), "Should have detected missing accounts");
}

#[test]
fn test_simulate_with_tracking_disabled() {
    let svm = LiteSVM::new();

    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to = Pubkey::new_unique();

    let mut svm_mut = svm.clone();
    svm_mut.airdrop(&from, 10_000).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_kp],
        Message::new(&[instruction], Some(&from)),
        svm_mut.latest_blockhash(),
    );

    let sim_result = svm_mut.simulate_transaction(tx).unwrap();

    // Tracking disabled, should be None
    assert!(sim_result.meta.accessed_accounts.is_none());
}

#[test]
fn test_simulate_with_tracking_enabled() {
    let svm = LiteSVM::new().with_account_tracking(true);

    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to = Pubkey::new_unique();

    let mut svm_mut = svm.clone();
    svm_mut.airdrop(&from, 10_000).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_kp],
        Message::new(&[instruction], Some(&from)),
        svm_mut.latest_blockhash(),
    );

    let sim_result = svm_mut.simulate_transaction(tx).unwrap();

    // Check that accessed accounts are included in simulation result
    let accessed = sim_result
        .meta
        .accessed_accounts
        .expect("Should have accessed accounts");

    assert!(accessed.contains(&from));
    assert!(accessed.contains(&to));
}

#[test]
fn test_multiple_transactions_tracking() {
    let mut svm = LiteSVM::new().with_account_tracking(true);

    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to1 = Pubkey::new_unique();
    let to2 = Pubkey::new_unique();

    svm.airdrop(&from, 100_000).unwrap();

    // First transaction
    let tx1 = Transaction::new(
        &[&from_kp],
        Message::new(&[transfer(&from, &to1, 64)], Some(&from)),
        svm.latest_blockhash(),
    );

    let result1 = svm.send_transaction(tx1).unwrap();
    let accessed1 = result1
        .accessed_accounts
        .expect("Should have accessed accounts");
    assert!(accessed1.contains(&from));
    assert!(accessed1.contains(&to1));
    assert!(!accessed1.contains(&to2)); // to2 not accessed yet

    // Second transaction
    let tx2 = Transaction::new(
        &[&from_kp],
        Message::new(&[transfer(&from, &to2, 64)], Some(&from)),
        svm.latest_blockhash(),
    );

    let result2 = svm.send_transaction(tx2).unwrap();
    let accessed2 = result2
        .accessed_accounts
        .expect("Should have accessed accounts");
    assert!(accessed2.contains(&from));
    assert!(accessed2.contains(&to2));
    // Should NOT contain to1 (tracking is per-transaction)
    assert!(!accessed2.contains(&to1));
}

#[test]
fn test_failed_transaction_with_tracking() {
    let mut svm = LiteSVM::new().with_account_tracking(true);

    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to = Pubkey::new_unique();

    // Don't airdrop, so transaction will fail with insufficient funds

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_kp],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );

    let failed = svm.send_transaction(tx).unwrap_err();

    // Even though transaction failed, we should still have tracking data
    let accessed = failed
        .meta
        .accessed_accounts
        .expect("Should have accessed accounts");

    // Should have tried to access the from account
    assert!(accessed.contains(&from));
}

#[test]
fn test_tracking_can_be_toggled() {
    // Create with tracking enabled
    let mut svm = LiteSVM::new().with_account_tracking(true);

    let from_kp = Keypair::new();
    let from = from_kp.pubkey();
    let to = Pubkey::new_unique();

    svm.airdrop(&from, 10_000).unwrap();

    let tx = Transaction::new(
        &[&from_kp],
        Message::new(&[transfer(&from, &to, 64)], Some(&from)),
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx).unwrap();
    assert!(result.accessed_accounts.is_some());

    // Now disable tracking by creating a new instance
    let mut svm2 = LiteSVM::new().with_account_tracking(false);

    let from_kp2 = Keypair::new();
    let from2 = from_kp2.pubkey();
    let to2 = Pubkey::new_unique();

    svm2.airdrop(&from2, 10_000).unwrap();

    let tx2 = Transaction::new(
        &[&from_kp2],
        Message::new(&[transfer(&from2, &to2, 64)], Some(&from2)),
        svm2.latest_blockhash(),
    );

    let result2 = svm2.send_transaction(tx2).unwrap();
    assert!(result2.accessed_accounts.is_none());
}
