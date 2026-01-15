use {
    litesvm::LiteSVM,
    solana_address::Address,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_native_token::LAMPORTS_PER_SOL,
    solana_signer::Signer,
    solana_system_interface::instruction::{allocate, create_account, transfer},
    solana_transaction::Transaction,
};

#[test_log::test]
fn system_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();

    let mut svm = LiteSVM::new();
    let expected_fee = 5000;
    svm.airdrop(&from, 100 + expected_fee).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    let from_account = svm.get_account(&from);
    let to_account = svm.get_account(&to);

    assert!(tx_res.is_ok());
    assert_eq!(from_account.unwrap().lamports, 36);
    assert_eq!(to_account.unwrap().lamports, 64);
}

#[test_log::test]
fn system_create_account() {
    let from_keypair = Keypair::new();
    let new_account = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    let expected_fee = 5000 * 2; // two signers
    let space = 10;
    let rent_amount = svm.minimum_balance_for_rent_exemption(space);
    let lamports = rent_amount + expected_fee;
    svm.airdrop(&from, lamports).unwrap();

    let instruction = create_account(
        &from,
        &new_account.pubkey(),
        rent_amount,
        space as u64,
        &solana_sdk_ids::system_program::id(),
    );
    let tx = Transaction::new(
        &[&from_keypair, &new_account],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    let account = svm.get_account(&new_account.pubkey()).unwrap();

    assert_eq!(account.lamports, rent_amount);
    assert_eq!(account.data.len(), space);
    assert_eq!(account.owner, solana_sdk_ids::system_program::id());
}

#[test_log::test]
fn system_allocate_account() {
    let from_keypair = Keypair::new();
    let new_account_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let new_account = new_account_keypair.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, 10 * LAMPORTS_PER_SOL).unwrap();

    let instruction = allocate(&new_account, 10);

    let tx = Transaction::new(
        &[&from_keypair, &new_account_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    assert!(svm.get_account(&new_account).is_none());
}

#[test_log::test]
fn test_airdrop_pubkey() {
    let funding_amount = 10 * LAMPORTS_PER_SOL;
    let mut svm = LiteSVM::new().with_lamports(funding_amount);

    let airdrop_pubkey = svm.airdrop_pubkey();

    let initial_balance = svm.get_balance(&airdrop_pubkey).unwrap();
    assert_eq!(initial_balance, funding_amount);

    let recipient = Address::new_unique();
    let airdrop_amount = 100_000;
    svm.airdrop(&recipient, airdrop_amount).unwrap();

    let after_airdrop = svm.get_balance(&airdrop_pubkey).unwrap();
    assert!(after_airdrop < initial_balance);
    assert_eq!(
        after_airdrop,
        initial_balance - airdrop_amount - 5000 // transaction fee
    );

    assert_eq!(svm.get_balance(&recipient).unwrap(), airdrop_amount);
    assert_eq!(svm.airdrop_pubkey(), airdrop_pubkey);

    let recipient2 = Address::new_unique();
    svm.airdrop(&recipient2, airdrop_amount).unwrap();
    assert_eq!(svm.get_balance(&recipient2).unwrap(), airdrop_amount);
}
