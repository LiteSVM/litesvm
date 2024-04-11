use litesvm::LiteSVM;
use solana_program::{message::Message, pubkey::Pubkey, system_instruction::transfer};
use solana_sdk::{
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction, TransactionError},
};

#[test_log::test]
fn test_invalid_blockhash() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();

    svm.airdrop(&from, svm.get_sysvar::<Rent>().minimum_balance(0))
        .unwrap();
    let instruction = transfer(&from, &to, 1);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        solana_sdk::hash::Hash::new_unique(),
    );
    let tx_res = svm.send_transaction(tx);

    assert_eq!(tx_res.unwrap_err().err, TransactionError::BlockhashNotFound);
}
