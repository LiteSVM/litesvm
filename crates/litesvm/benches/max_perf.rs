use {
    criterion::{criterion_group, criterion_main, Criterion},
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::path::PathBuf,
};

const NUM_GREETINGS: u8 = 255;

fn make_tx(
    program_id: Address,
    counter_address: Address,
    payer_pk: &Address,
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

fn criterion_benchmark(c: &mut Criterion) {
    let mut svm = LiteSVM::new()
        .with_blockhash_check(false)
        .with_sigverify(false)
        .with_transaction_history(0);
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Address::new_unique();
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    svm.add_program_from_file(program_id, &so_path).unwrap();
    svm.airdrop(&payer_pk, 100_000_000_000).unwrap();
    let counter_address = Address::new_unique();
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
}

fn counter_acc(program_id: Address) -> solana_account::Account {
    Account {
        lamports: 5,
        data: vec![0_u8; std::mem::size_of::<u32>()],
        owner: program_id,
        ..Default::default()
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
