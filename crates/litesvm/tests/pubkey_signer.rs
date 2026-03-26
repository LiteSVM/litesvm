use {
    jupnet_sdk::{
        message::Message, native_token::MOTES_PER_JUP, pubkey::Pubkey, signature::TypedSignature,
        system_instruction::transfer, transaction::Transaction,
    },
    litesvm::LiteSVM,
};

#[test]
fn pubkey_signer() {
    let mut svm = LiteSVM::new().with_sigverify(false);

    let dean = Pubkey::new_unique();
    svm.airdrop(&dean, 10 * MOTES_PER_JUP).unwrap();
    let jacob = Pubkey::new_unique();

    let ix = transfer(&dean, &jacob, MOTES_PER_JUP);
    let hash = svm.latest_blockhash();
    let tx = Transaction {
        message: Message::new_with_blockhash(&[ix], Some(&dean), &hash),
        signatures: vec![TypedSignature::default()],
    };
    svm.send_transaction(tx).unwrap();

    svm.expire_blockhash();

    let ix = transfer(&dean, &jacob, MOTES_PER_JUP);
    let hash = svm.latest_blockhash();
    let tx = Transaction {
        message: Message::new_with_blockhash(&[ix], Some(&dean), &hash),
        signatures: vec![TypedSignature::default()],
    };
    svm.send_transaction(tx).unwrap();

    assert!(svm.get_balance(&dean).unwrap() < 8 * MOTES_PER_JUP);
    assert_eq!(svm.get_balance(&jacob).unwrap(), 2 * MOTES_PER_JUP);
}
