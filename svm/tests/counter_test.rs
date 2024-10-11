use std::path::PathBuf;

use litesvm::LiteSVM;
use solana_program::address_lookup_table::instruction::create_lookup_table;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::message::VersionedMessage;
use solana_program::{
    address_lookup_table::instruction::extend_lookup_table,
    instruction::{AccountMeta, Instruction},
    message::{v0::Message as MessageV0, Message},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk::transaction::{TransactionError, VersionedTransaction};
use solana_sdk::{
    account::Account,
    pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};

const NUM_GREETINGS: u8 = 127;

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
    svm.add_program(program_id, &read_counter_program());
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
    blockhash: solana_program::hash::Hash,
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

fn add_program(bytes: &[u8], program_id: Pubkey, pt: &mut solana_program_test::ProgramTest) {
    pt.add_account(
        program_id,
        Account {
            lamports: Rent::default().minimum_balance(bytes.len()).max(1),
            data: bytes.to_vec(),
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );
}

fn counter_acc(program_id: Pubkey) -> solana_sdk::account::Account {
    Account {
        lamports: 5,
        data: vec![0_u8; std::mem::size_of::<u32>()],
        owner: program_id,
        ..Default::default()
    }
}

async fn do_program_test(program_id: Pubkey, counter_address: Pubkey) {
    let mut pt = solana_program_test::ProgramTest::default();
    add_program(&read_counter_program(), program_id, &mut pt);
    let mut ctx = pt.start_with_context().await;
    ctx.set_account(&counter_address, &counter_acc(program_id).into());
    assert_eq!(
        ctx.banks_client
            .get_account(counter_address)
            .await
            .unwrap()
            .unwrap()
            .data,
        0u32.to_le_bytes().to_vec()
    );
    assert!(ctx
        .banks_client
        .get_account(program_id)
        .await
        .unwrap()
        .is_some());

    for deduper in 0..NUM_GREETINGS {
        let tx = make_tx(
            program_id,
            counter_address,
            &ctx.payer.pubkey(),
            ctx.last_blockhash,
            &ctx.payer,
            deduper,
        );
        let tx_res = ctx
            .banks_client
            .process_transaction_with_metadata(tx)
            .await
            .unwrap();
        tx_res.result.unwrap();
    }
    let fetched = ctx
        .banks_client
        .get_account(counter_address)
        .await
        .unwrap()
        .unwrap()
        .data[0];
    assert_eq!(fetched, NUM_GREETINGS);
}

#[test]
fn banks_client_test() {
    let program_id = Pubkey::new_unique();

    let counter_address = Pubkey::new_unique();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { do_program_test(program_id, counter_address).await });
}

fn make_tx_wrong_signature(
    program_id: Pubkey,
    counter_address: Pubkey,
    payer_pk: &Pubkey,
    blockhash: solana_program::hash::Hash,
    payer_kp: &Keypair,
) -> Transaction {
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0, 0],
        }],
        Some(payer_pk),
        &blockhash,
    );
    let mut tx = Transaction::new(&[&payer_kp], msg, blockhash);
    tx.signatures[0] = Signature::new_unique();
    tx
}

async fn do_program_test_wrong_signature(program_id: Pubkey, counter_address: Pubkey) {
    let mut pt = solana_program_test::ProgramTest::default();
    add_program(&read_counter_program(), program_id, &mut pt);
    let mut ctx = pt.start_with_context().await;
    ctx.set_account(&counter_address, &counter_acc(program_id).into());
    assert_eq!(
        ctx.banks_client
            .get_account(counter_address)
            .await
            .unwrap()
            .unwrap()
            .data,
        0u32.to_le_bytes().to_vec()
    );
    assert!(ctx
        .banks_client
        .get_account(program_id)
        .await
        .unwrap()
        .is_some());

    let tx = make_tx_wrong_signature(
        program_id,
        counter_address,
        &ctx.payer.pubkey(),
        ctx.last_blockhash,
        &ctx.payer,
    );
    let tx_res = ctx
        .banks_client
        .process_transaction_with_metadata(tx)
        .await
        .unwrap();
    tx_res.result.unwrap();
    let fetched = ctx
        .banks_client
        .get_account(counter_address)
        .await
        .unwrap()
        .unwrap()
        .data[0];
    assert_eq!(fetched, 1);
}

/// Confirm that process_transaction_with_metadata
/// does not do sigverify.
#[test]
fn test_process_transaction_with_metadata_wrong_signature() {
    let program_id = Pubkey::new_unique();

    let counter_address = Pubkey::new_unique();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { do_program_test_wrong_signature(program_id, counter_address).await });
}

#[test]
fn test_address_lookup_table() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = pubkey!("GtdambwDgHWrDJdVPBkEHGhCwokqgAoch162teUjJse2");
    svm.add_program(program_id, &read_counter_program());
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
