use {
    litesvm::LiteSVM, solana_keypair::Keypair, solana_program_pack::Pack, solana_rent::Rent,
    solana_signer::Signer, solana_transaction::Transaction, spl_token_interface::state::Mint,
};

#[test]
fn spl_token() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let mint_len = Mint::LEN;

    svm.airdrop(&payer_pk, 1000000000).unwrap();

    let create_acc_ins = solana_system_interface::instruction::create_account(
        &payer_pk,
        &mint_pk,
        svm.minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &spl_token_interface::ID,
    );

    let init_mint_ins = spl_token_interface::instruction::initialize_mint2(
        &spl_token_interface::ID,
        &mint_pk,
        &payer_pk,
        None,
        8,
    )
    .unwrap();
    let balance_before = svm.get_balance(&payer_pk).unwrap();
    let expected_fee = 2 * 5000; // two signers
    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_acc_ins, init_mint_ins],
        Some(&payer_pk),
        &[&payer_kp, &mint_kp],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok());
    let expected_rent = svm.get_sysvar::<Rent>().minimum_balance(Mint::LEN);
    let balance_after = svm.get_balance(&payer_pk).unwrap();

    assert_eq!(balance_before - balance_after, expected_rent + expected_fee);

    let mint_acc = svm.get_account(&mint_kp.pubkey());
    let mint = Mint::unpack(&mint_acc.unwrap().data).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.mint_authority, Some(payer_pk).into());
}
