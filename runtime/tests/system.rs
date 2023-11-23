use solana_program::{
    message::{Message, VersionedMessage},
    pubkey::Pubkey,
    system_instruction::{create_account, transfer},
};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::VersionedTransaction};

use light_sol_bankrun::bank::LightBank;

#[test]
pub fn system_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let bank = LightBank::new();

    bank.airdrop(&from, 100).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(Message::new(&[instruction], Some(&from))),
        &[&from_keypair],
    )
    .unwrap();

    let tx_result = bank.execute_transaction(tx).unwrap();

    let from_account = bank.get_account(from);
    let to_account = bank.get_account(to);

    assert!(tx_result.result.is_ok());
    assert_eq!(from_account.lamports, 36);
    assert_eq!(to_account.lamports, 64);
}

#[test]
pub fn system_create_account() {
    let from_keypair = Keypair::new();
    let new_account = Keypair::new();
    let from = from_keypair.pubkey();

    let bank = LightBank::new();

    let lamports = bank.get_minimum_balance_for_rent_exemption(10);

    // bank.airdrop(&from, lamports).unwrap();

    let instruction = create_account(
        &from,
        &new_account.pubkey(),
        0,
        10,
        &solana_program::system_program::id(),
    );
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(Message::new(&[instruction], Some(&from))),
        &[&from_keypair, &new_account],
    )
    .unwrap();

    let tx_result = bank.execute_transaction(tx).unwrap();

    println!("{tx_result:?}");
    assert!(tx_result.result.is_ok());
}
