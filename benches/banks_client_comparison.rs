use criterion::{criterion_group, criterion_main, Criterion};
use litesvm::LiteSVM;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk::{
    account::Account, message::Message, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

const COUNTER_PROGRAM_BYTES: &[u8] = include_bytes!("../tests/programs_bytes/counter.so");

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

async fn do_program_test(program_id: Pubkey, counter_address: Pubkey) {
    let mut pt = solana_program_test::ProgramTest::default();
    add_program(COUNTER_PROGRAM_BYTES, program_id, &mut pt);
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

fn criterion_benchmark(c: &mut Criterion) {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::new_unique();

    svm.store_program(program_id, COUNTER_PROGRAM_BYTES);
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let counter_address = Pubkey::new_unique();
    let mut group = c.benchmark_group("comparison");
    group.bench_function("litesvm_bench", |b| {
        b.iter(|| {
            svm.set_account(counter_address, counter_acc(program_id));
            for deduper in 0..NUM_GREETINGS {
                let tx = make_tx(
                    program_id,
                    counter_address,
                    &payer_pk,
                    svm.latest_blockhash(),
                    &payer_kp,
                    deduper,
                );
                let _ = svm.send_transaction(tx.clone());
                svm.expire_blockhash();
            }
            assert_eq!(svm.get_account(&counter_address).data[0], NUM_GREETINGS);
        })
    });
    group.bench_function("banks_client_bench", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                do_program_test(program_id, counter_address).await;
            });
        })
    });
}

fn counter_acc(program_id: Pubkey) -> solana_sdk::account::Account {
    Account {
        lamports: 5,
        data: vec![0_u8; std::mem::size_of::<u32>()],
        owner: program_id,
        ..Default::default()
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
