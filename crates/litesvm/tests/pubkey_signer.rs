use {
    litesvm::LiteSVM, solana_message::Message, solana_native_token::LAMPORTS_PER_SOL,
    solana_pubkey::Pubkey, solana_signature::Signature,
    solana_system_interface::instruction::transfer, solana_transaction::Transaction,
};

#[test]
fn pubkey_signer() {
    let mut svm = LiteSVM::new().with_sigverify(false);

    let dean = Pubkey::new_unique();
    svm.airdrop(&dean, 10 * LAMPORTS_PER_SOL).unwrap();
    let jacob = Pubkey::new_unique();

    let ix = transfer(&dean, &jacob, LAMPORTS_PER_SOL);
    let hash = svm.latest_blockhash();
    let tx = Transaction {
        message: Message::new_with_blockhash(&[ix], Some(&dean), &hash),
        signatures: vec![Signature::default()],
    };
    svm.send_transaction(tx).unwrap();

    assert!(svm.get_balance(&dean).unwrap() < 9 * LAMPORTS_PER_SOL);
    assert_eq!(svm.get_balance(&jacob).unwrap(), LAMPORTS_PER_SOL);
}
