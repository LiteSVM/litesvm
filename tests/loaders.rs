use litesvm::LiteSVM;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    message::Message,
};
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

use crate::programs_bytes::HELLO_WORLD_BYTES;

mod programs_bytes;

#[test]
fn hello_world_with_store() {
    let mut bank = LiteSVM::new();

    let payer = Keypair::new();
    let program_bytes = HELLO_WORLD_BYTES;

    bank.airdrop(&payer.pubkey(), 1000000000).unwrap();

    let program_kp = Keypair::new();
    let program_id = program_kp.pubkey();
    bank.add_program(program_id, program_bytes);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &[],
        vec![AccountMeta::new(payer.pubkey(), true)],
    );
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], message, bank.latest_blockhash());
    let tx_result = bank.send_transaction(tx);

    assert!(tx_result.is_ok());
    assert!(tx_result
        .unwrap()
        .logs
        .contains(&"Program log: Hello world!".to_string()));
}

#[test]
fn hello_world_with_deploy_upgradeable() {
    let mut bank = LiteSVM::new();

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_bytes = HELLO_WORLD_BYTES;

    bank.airdrop(&payer_pk, 10000000000).unwrap();

    let program_keypair = Keypair::new();
    bank.deploy_upgradeable_program(&payer_kp, &program_keypair, program_bytes)
        .unwrap();
    let program_id = program_keypair.pubkey();
    let instruction =
        Instruction::new_with_bytes(program_id, &[], vec![AccountMeta::new(payer_pk, true)]);
    let message = Message::new(&[instruction], Some(&payer_pk));
    let tx = Transaction::new(&[&payer_kp], message, bank.latest_blockhash());
    let tx_result = bank.send_transaction(tx);
    assert!(tx_result
        .unwrap()
        .logs
        .contains(&"Program log: Hello world!".to_string()));
}
