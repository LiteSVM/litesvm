use {
    criterion::{criterion_group, criterion_main, BenchmarkId, Criterion},
    litesvm::LiteSVM,
    solana_account::Account,
    solana_address::Address,
    solana_clock::Clock,
};

/// Builds an SVM pre-seeded with `num_accounts` dummy accounts on top of the
/// accounts LiteSVM creates for itself.
fn svm_with_accounts(num_accounts: usize) -> LiteSVM {
    let mut svm = LiteSVM::new();
    for i in 0..num_accounts {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&(i as u64).to_le_bytes());
        svm.set_account(
            Address::new_from_array(bytes),
            Account {
                lamports: 1_000_000,
                data: vec![0u8; 32],
                owner: solana_sdk_ids::system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
    }
    svm
}

/// Writing the clock sysvar should cost the same no matter how many accounts
/// the SVM holds.
fn bench_set_clock(c: &mut Criterion) {
    let mut group = c.benchmark_group("set_clock");
    for num_accounts in [1usize, 1_000, 10_000] {
        let mut svm = svm_with_accounts(num_accounts);
        let mut clock = svm.get_sysvar::<Clock>();
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &num_accounts,
            |b, _| {
                b.iter(|| {
                    clock.slot += 1;
                    svm.set_sysvar(&clock);
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_set_clock);
criterion_main!(benches);
