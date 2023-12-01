use solana_program::{
    message::Message,
    pubkey::Pubkey,
    system_instruction::{create_account, transfer},
};
use solana_sdk::{signature::Keypair, signer::Signer};

use light_sol_bankrun::bank::LightBank;

#[test]
pub fn system_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut bank = LightBank::new();

    bank.airdrop(&from, 100).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = bank
        .send_message(Message::new(&[instruction], Some(&from)), &[&from_keypair])
        .unwrap();

    let from_account = bank.get_account(&from);
    let to_account = bank.get_account(&to);

    assert!(tx.result.is_ok());
    assert_eq!(from_account.lamports, 36);
    assert_eq!(to_account.lamports, 64);
}

#[test]
pub fn system_create_account() {
    let from_keypair = Keypair::new();
    let new_account = Keypair::new();
    let from = from_keypair.pubkey();

    let mut bank = LightBank::new();

    let lamports = bank.get_minimum_balance_for_rent_exemption(10);
    bank.airdrop(&from, lamports).unwrap();

    let instruction = create_account(
        &from,
        &new_account.pubkey(),
        lamports,
        10,
        &solana_program::system_program::id(),
    );
    let tx = bank
        .send_message(
            Message::new(&[instruction], Some(&from)),
            &[&from_keypair, &new_account],
        )
        .unwrap();

    let account = bank.get_account(&new_account.pubkey());

    assert!(tx.result.is_ok());
    assert_eq!(account.lamports, lamports);
    assert_eq!(account.data.len(), 10);
    assert_eq!(account.owner, solana_program::system_program::id());
}
