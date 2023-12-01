use light_sol_bankrun::{bank::LightBank, deploy_program};
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
    let result = bank.send_message(message, &[&payer, &mint_kp]);

    println!("{result:?}");
    assert_eq!(1, 2);
}

#[test]
pub fn hello_world() {
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
    let result = bank.send_message(message, &[&payer]);

    println!("{result:?}");
    assert_eq!(1, 2);
}
