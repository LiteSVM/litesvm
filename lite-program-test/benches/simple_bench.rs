use criterion::{criterion_group, criterion_main, Criterion};
use lite_program_test::ProgramTest;
use solana_sdk::{
    program_pack::Pack, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

fn criterion_benchmark(c: &mut Criterion) {
    let program_test = ProgramTest::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let mint_len = spl_token::state::Mint::LEN;

    program_test.request_airdrop(&payer_pk, 1000000000);

    let create_acc_ins = system_instruction::create_account(
        &payer_pk,
        &mint_pk,
        program_test.get_minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &spl_token::id(),
    );

    let init_mint_ins =
        spl_token::instruction::initialize_mint2(&spl_token::id(), &mint_pk, &payer_pk, None, 8)
            .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_acc_ins, init_mint_ins],
        Some(&payer_pk),
        &[&payer_kp, &mint_kp],
        program_test.get_latest_blockhash(),
    );
    c.bench_function("simple_bench", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let tx_result = program_test.send_transaction(tx.clone());
                assert!(tx_result.is_ok())
            }
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
