use lite_program_test::ProgramTest;
use solana_sdk::{
    program_pack::Pack, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

#[test]
pub fn spl_token() {
    let program_test = ProgramTest::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let mint_len = spl_token::state::Mint::LEN;

    program_test.request_airdrop(&payer_pk, 1000000000);

    let create_acc_ins = system_instruction::create_account(
        &payer_pk,
        &mint_pk,
        program_test.get_minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &spl_token::id(),
    );

    let init_mint_ins =
        spl_token::instruction::initialize_mint2(&spl_token::id(), &mint_pk, &payer_pk, None, 8)
            .unwrap();

    let tx_result = program_test
        .send_transaction(Transaction::new_signed_with_payer(
            &[create_acc_ins, init_mint_ins],
            Some(&payer_pk),
            &[&payer_kp, &mint_kp],
            program_test.get_latest_blockhash(),
        ))
        .unwrap();

    assert!(tx_result.result.is_ok());

    let mint_acc = program_test.get_account(&mint_kp.pubkey());
    let mint = spl_token::state::Mint::unpack(&mint_acc.data).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.mint_authority, Some(payer_pk).into());
}
