use {
    jupnet_sdk::{
        account::Account,
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::{keypair::Keypair, Signer},
        transaction::{Transaction, TransactionError},
    },
    litesvm::LiteSVM,
    std::path::PathBuf,
};

fn read_counter_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    std::fs::read(so_path).unwrap()
}

#[test]
pub fn integration_test() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::from_str_const("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_counter_program())
        .unwrap();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = Pubkey::from_str_const("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
    let _ = svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    );
    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        0u32.to_le_bytes().to_vec()
    );
    let num_greets = 2u8;
    for deduper in 0..num_greets {
        let tx = make_tx(
            program_id,
            counter_address,
            &payer_pk,
            blockhash,
            &payer_kp,
            deduper,
        );
        let _ = svm.send_transaction(tx).unwrap();
    }
    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        (num_greets as u32).to_le_bytes().to_vec()
    );
}

fn make_tx(
    program_id: Pubkey,
    counter_address: Pubkey,
    payer_pk: &Pubkey,
    blockhash: Hash,
    payer_kp: &Keypair,
    deduper: u8,
) -> Transaction {
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0, deduper],
        }],
        Some(payer_pk),
        &blockhash,
    );
    Transaction::new(&[payer_kp], msg, blockhash)
}

#[test]
pub fn test_nonexistent_program() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::from_str_const("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = Pubkey::from_str_const("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
    svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    )
    .unwrap();
    let tx = make_tx(
        program_id,
        counter_address,
        &payer_pk,
        blockhash,
        &payer_kp,
        0,
    );
    let err = svm.send_transaction(tx).unwrap_err();
    assert_eq!(err.err, TransactionError::InvalidProgramForExecution);
}
