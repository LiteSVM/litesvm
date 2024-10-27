use litesvm::LiteSVM;
use solana_program::address_lookup_table::instruction::{create_lookup_table, extend_lookup_table};
use solana_sdk::{
    message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction,
};

#[test]
fn test_inner_instruction_parsing() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let (lookup_table_ix, lookup_table_address) = create_lookup_table(payer_pk, payer_pk, 0);
    let extend_ix = extend_lookup_table(
        lookup_table_address,
        payer_pk,
        Some(payer_pk),
        vec![Pubkey::new_unique()],
    );
    let lookup_msg = Message::new(&[lookup_table_ix, extend_ix], Some(&payer_pk));
    let lookup_tx = Transaction::new(&[&payer_kp], lookup_msg, blockhash);
    let result = svm.send_transaction(lookup_tx).unwrap();
    assert_eq!(2, result.inner_instructions.len());
    assert_eq!(3, result.inner_instructions[0].len());
    assert_eq!(1, result.inner_instructions[1].len());
    assert_eq!(2, result.inner_instructions[0][0].stack_height);
    assert_eq!(
        2,
        result.inner_instructions[0][0].instruction.program_id_index,
    );
    assert_eq!(
        vec![0, 1],
        result.inner_instructions[0][0].instruction.accounts
    );
    assert_eq!(
        vec![2, 0, 0, 0, 128, 138, 19, 0, 0, 0, 0, 0],
        result.inner_instructions[0][0].instruction.data
    );
}
