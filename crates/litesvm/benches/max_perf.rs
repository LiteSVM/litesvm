use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, Criterion};
use litesvm::LiteSVM;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk::{
    account::Account, message::Message, rent::Rent, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

const NUM_GREETINGS: u8 = 255;

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

fn criterion_benchmark(c: &mut Criterion) {
    let mut svm = LiteSVM::new()
        .with_blockhash_check(false)
        .with_sigverify(false)
        .with_transaction_history(0);
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::new_unique();
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    svm.add_program_from_file(program_id, &so_path).unwrap();
    svm.airdrop(&payer_pk, 100_000_000_000).unwrap();
    let counter_address = Pubkey::new_unique();
    let latest_blockhash = svm.latest_blockhash();
    let tx = make_tx(
        program_id,
        counter_address,
        &payer_pk,
        latest_blockhash,
        &payer_kp,
        0,
    );
    let mut group = c.benchmark_group("max_perf_comparison");
    group.bench_function("max_perf_litesvm", |b| {
        b.iter(|| {
            let _ = svm.set_account(counter_address, counter_acc(program_id));
            for _ in 0..NUM_GREETINGS {
                svm.send_transaction(tx.clone()).unwrap();
            }
            assert_eq!(
                svm.get_account(&counter_address).unwrap().data[0],
                NUM_GREETINGS
            );
        })
    });
    group.bench_function("max_perf_banks_client", |b| {
        // this has to do more work than max_perf_litesvm because you can't turn off blockhash checking.
        // That's ok, the point is that litesvm lets you strip away more stuff.
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                do_program_test(program_id, counter_address).await;
            });
        })
    });
}

async fn do_program_test(program_id: Pubkey, counter_address: Pubkey) {
    let mut pt = solana_program_test::ProgramTest::default();
    add_program(&read_counter_program(), program_id, &mut pt);
    let mut ctx = pt.start_with_context().await;
    ctx.set_account(&counter_address, &counter_acc(program_id).into());

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
            .process_transaction_with_metadata(tx.clone())
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

fn counter_acc(program_id: Pubkey) -> solana_sdk::account::Account {
    Account {
        lamports: 5,
        data: vec![0_u8; std::mem::size_of::<u32>()],
        owner: program_id,
        ..Default::default()
    }
}

fn read_counter_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    std::fs::read(so_path).unwrap()
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

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
