use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::address,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_sdk_ids::loader_v4,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

fn read_counter_program() -> Vec<u8> {
    include_bytes!("../../node-litesvm/program_bytes/counter.so").to_vec()
}

#[test_log::test]
fn add_program_with_loader_v4_executes_program() {
    let program_id = address!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    let counter_address = address!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");

    let payer = Keypair::new();
    let payer_address = payer.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&payer_address, 1_000_000_000).unwrap();

    svm.add_program_with_loader(program_id, &read_counter_program(), loader_v4::id())
        .unwrap();

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

    let tx = Transaction::new(
        &[&payer],
        Message::new_with_blockhash(
            &[Instruction {
                program_id,
                accounts: vec![AccountMeta::new(counter_address, false)],
                data: vec![0],
            }],
            Some(&payer_address),
            &svm.latest_blockhash(),
        ),
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).unwrap();

    assert_eq!(
        svm.get_account(&counter_address).unwrap().data,
        1u32.to_le_bytes().to_vec()
    );
}
