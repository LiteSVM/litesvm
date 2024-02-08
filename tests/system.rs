use lite_svm::LiteSVM;
use solana_program::{
    message::Message,
    pubkey::Pubkey,
    system_instruction::{create_account, transfer},
};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

#[test]
pub fn system_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut bank = LiteSVM::new();

    bank.airdrop(&from, 100).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        bank.latest_blockhash(),
    );
    let tx_res = bank.send_transaction(tx).unwrap();

    let from_account = bank.get_account(&from);
    let to_account = bank.get_account(&to);

    assert!(tx_res.result.is_ok());
    assert_eq!(from_account.lamports, 36);
    assert_eq!(to_account.lamports, 64);
}

#[test]
pub fn system_create_account() {
    let from_keypair = Keypair::new();
    let new_account = Keypair::new();
    let from = from_keypair.pubkey();

    let mut bank = LiteSVM::new();

    let lamports = bank.minimum_balance_for_rent_exemption(10);
    bank.airdrop(&from, lamports).unwrap();

    let instruction = create_account(
        &from,
        &new_account.pubkey(),
        lamports,
        10,
        &solana_program::system_program::id(),
    );
    let tx = Transaction::new(
        &[&from_keypair, &new_account],
        Message::new(&[instruction], Some(&from)),
        bank.latest_blockhash(),
    );
    let tx_res = bank.send_transaction(tx).unwrap();

    let account = bank.get_account(&new_account.pubkey());

    assert!(tx_res.result.is_ok());
    assert_eq!(account.lamports, lamports);
    assert_eq!(account.data.len(), 10);
    assert_eq!(account.owner, solana_program::system_program::id());
}
