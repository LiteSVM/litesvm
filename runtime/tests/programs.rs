use light_sol_bankrun::{bank::LightBank, deploy_program, deploy_upgradeable_program};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    program_pack::Pack,
    system_instruction,
};
use solana_sdk::{signature::Keypair, signer::Signer};

use crate::programs_bytes::{HELLO_WORLD_BYTES, SPL_TOKEN_BYTES};

mod programs_bytes;

#[test]
pub fn spl_token() {
    let mut bank = LightBank::new();

    let payer = Keypair::new();
    let program_bytes = SPL_TOKEN_BYTES;
    bank.store_program(spl_token::id(), program_bytes);
    bank.airdrop(&payer.pubkey(), 1000000000).unwrap();
    let mint_kp = Keypair::new();

    let mint_len = spl_token::state::Mint::LEN;
    let create_acc_ins = system_instruction::create_account(
        &payer.pubkey(),
        &mint_kp.pubkey(),
        bank.get_minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &spl_token::id(),
    );

    let init_mint_ins = spl_token::instruction::initialize_mint2(
        &spl_token::id(),
        &mint_kp.pubkey(),
        &payer.pubkey(),
        None,
        8,
    )
    .unwrap();

    let message = Message::new(&[create_acc_ins, init_mint_ins], Some(&payer.pubkey()));
    let result = bank.send_message(message, &[&payer, &mint_kp]).unwrap();

    assert!(result.result.is_ok());

    let mint_acc = bank.get_account(&mint_kp.pubkey());
    let mint = spl_token::state::Mint::unpack(&mint_acc.data).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.mint_authority, Some(payer.pubkey()).into());
}

#[test]
pub fn hello_world_with_deploy() {
    let mut bank = LightBank::new();

    let payer = Keypair::new();
    let program_bytes = HELLO_WORLD_BYTES;

    bank.airdrop(&payer.pubkey(), 1000000000).unwrap();

    let program_id = deploy_program(&mut bank, &payer, program_bytes).unwrap();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &[],
        vec![AccountMeta::new(payer.pubkey(), true)],
    );
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let tx_result = bank.send_message(message, &[&payer]).unwrap();

    assert!(tx_result.result.is_ok());
    assert!(tx_result
        .metadata
        .logs
        .contains(&"Program log: Hello world!".to_string()));
}

#[test]
pub fn hello_world_with_deploy_upgradeable() {
    let mut bank = LightBank::new();

    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_bytes = HELLO_WORLD_BYTES;

    bank.airdrop(&payer_pk, 10000000000).unwrap();

    let program_id = deploy_upgradeable_program(&mut bank, &payer_kp, program_bytes).unwrap();

    let instruction =
        Instruction::new_with_bytes(program_id, &[], vec![AccountMeta::new(payer_pk, true)]);
    let message = Message::new(&[instruction], Some(&payer_pk));
    let tx_result = bank.send_message(message, &[&payer_kp]).unwrap();

    assert!(tx_result.result.is_ok());
    assert!(tx_result
        .metadata
        .logs
        .contains(&"Program log: Hello world!".to_string()));
}
