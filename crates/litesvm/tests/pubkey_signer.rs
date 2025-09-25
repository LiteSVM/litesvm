use {
    agave_feature_set::FeatureSet, litesvm::LiteSVM, solana_message::Message,
    solana_native_token::LAMPORTS_PER_SOL, solana_pubkey::Pubkey, solana_signature::Signature,
    solana_system_interface::instruction::transfer, solana_transaction::Transaction,
};

#[test]
fn pubkey_signer() {
    let mut svm = LiteSVM::default()
        .with_feature_set(FeatureSet::all_enabled())
        .with_builtins()
        .with_lamports(1_000_000u64.wrapping_mul(LAMPORTS_PER_SOL))
        .with_sysvars()
        .with_precompiles()
        .with_default_programs()
        .with_sigverify(false)
        .with_blockhash_check(true);

    let dean = Pubkey::new_unique();
    svm.airdrop(&dean, 10 * LAMPORTS_PER_SOL).unwrap();
    let jacob = Pubkey::new_unique();

    let ix = transfer(&dean, &jacob, 1 * LAMPORTS_PER_SOL);
    let hash = svm.latest_blockhash();
    let tx = Transaction {
        message: Message::new_with_blockhash(&[ix], Some(&dean), &hash),
        signatures: vec![Signature::default()],
    };
    svm.send_transaction(tx).unwrap();

    assert!(svm.get_balance(&dean).unwrap() < 9 * LAMPORTS_PER_SOL);
    assert_eq!(svm.get_balance(&jacob).unwrap(), 1 * LAMPORTS_PER_SOL);
}
