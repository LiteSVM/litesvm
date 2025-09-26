use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address_lookup_table_interface::instruction::{
        create_lookup_table, extend_lookup_table,
    },
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{
        v0::Message as MessageV0, AddressLookupTableAccount, Message, VersionedMessage,
    },
    solana_pubkey::{pubkey, Pubkey},
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    solana_transaction_error::TransactionError,
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
    let program_id = pubkey!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_counter_program())
        .unwrap();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = pubkey!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
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
    blockhash: solana_hash::Hash,
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
fn test_address_lookup_table() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = pubkey!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_counter_program())
        .unwrap();
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = pubkey!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
    let _ = svm.set_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        },
    );
    let (lookup_table_ix, lookup_table_address) = create_lookup_table(payer_pk, payer_pk, 0);
    let extend_ix = extend_lookup_table(
        lookup_table_address,
        payer_pk,
        Some(payer_pk),
        vec![counter_address],
    );
    let lookup_msg = Message::new(&[lookup_table_ix, extend_ix], Some(&payer_pk));
    let lookup_tx = Transaction::new(&[&payer_kp], lookup_msg, blockhash);
    svm.send_transaction(lookup_tx).unwrap();
    let alta = AddressLookupTableAccount {
        key: lookup_table_address,
        addresses: vec![counter_address],
    };
    let counter_msg = MessageV0::try_compile(
        &payer_pk,
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0, 0],
        }],
        &[alta],
        blockhash,
    )
    .unwrap();
    let counter_tx =
        VersionedTransaction::try_new(VersionedMessage::V0(counter_msg), &[&payer_kp]).unwrap();
    svm.warp_to_slot(1); // can't use the lookup table in the same slot
    svm.send_transaction(counter_tx).unwrap();
}

#[test]
pub fn test_nonexistent_program() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = pubkey!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let blockhash = svm.latest_blockhash();
    let counter_address = pubkey!("J39wvrFY2AkoAUCke5347RMNk3ditxZfVidoZ7U6Fguf");
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
