use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, Criterion};
use litesvm::LiteSVM;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk::{
    account::Account, message::Message, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

fn read_counter_program() -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push("test_programs/target/deploy/counter.so");
    std::fs::read(so_path).unwrap()
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

fn criterion_benchmark(c: &mut Criterion) {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::new_unique();

    svm.add_program(program_id, &read_counter_program());
    svm.airdrop(&payer_pk, 1000000000).unwrap();
    let counter_address = Pubkey::new_unique();
    c.bench_function("simple_bench", |b| {
        b.iter(|| {
            let _ = svm.set_account(counter_address, counter_acc(program_id));
            svm.expire_blockhash();
            let latest_blockhash = svm.latest_blockhash();
            for deduper in 0..NUM_GREETINGS {
                let tx = make_tx(
                    program_id,
                    counter_address,
                    &payer_pk,
                    latest_blockhash,
                    &payer_kp,
                    deduper,
                );
                svm.send_transaction(tx).unwrap();
            }
            assert_eq!(
                svm.get_account(&counter_address).unwrap().data[0],
                NUM_GREETINGS
            );
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
