use lite_program_test::ProgramTest;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer, transaction::Transaction};

const COUNTER_PROGRAM_BYTES: &[u8] =
    include_bytes!("../../../../lite-svm/tests/programs_bytes/counter.so");

#[test]
pub fn integration_test() {
    let svm = ProgramTest::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::new_unique();
    svm.store_program(program_id, COUNTER_PROGRAM_BYTES);

    svm.request_airdrop(&payer_pk, 1000000000);
    let blockhash = svm.get_latest_blockhash();
    let counter_address = Pubkey::new_unique();
    svm.get_bank_mut().accounts.add_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        }
        .into(),
    );
    assert_eq!(
        svm.get_account(&counter_address).data,
        0u32.to_le_bytes().to_vec()
    );
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0],
        }],
        Some(&payer_pk),
        &blockhash,
    );
    let tx = Transaction::new(&[&payer_kp], msg, blockhash);
    svm.send_transaction(tx).unwrap();
    assert_eq!(
        svm.get_account(&counter_address).data,
        1u32.to_le_bytes().to_vec()
    );
}
