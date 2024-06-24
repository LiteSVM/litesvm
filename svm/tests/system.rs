use litesvm::LiteSVM;
use solana_program::{
    message::Message,
    pubkey::Pubkey,
    system_instruction::{create_account, transfer},
};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

#[test_log::test]
fn system_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

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
        &solana_program::system_program::id(),
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
    assert_eq!(account.owner, solana_program::system_program::id());
}
