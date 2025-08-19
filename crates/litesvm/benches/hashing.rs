use {
    criterion::{criterion_group, criterion_main, Criterion},
    solana_pubkey::Pubkey,
    std::{
        hash::{BuildHasher, BuildHasherDefault, DefaultHasher, Hash, Hasher},
        hint::black_box,
    },
};

#[inline(never)]
fn std_default(address: &Pubkey, hash_builder: &BuildHasherDefault<DefaultHasher>) -> u64 {
    let mut hasher = hash_builder.build_hasher();
    address.hash(&mut hasher);
    hasher.finish()
}

#[cfg(feature = "hashbrown")]
#[inline(never)]
fn hashbrown(address: &Pubkey, hash_builder: &hashbrown::DefaultHashBuilder) -> u64 {
    let mut hasher = hash_builder.build_hasher();
    address.hash(&mut hasher);
    hasher.finish()
}

fn criterion_benchmark(c: &mut Criterion) {
    let address = Pubkey::new_unique();

    let mut group = c.benchmark_group("hashers");

    group.bench_function("default", |b| {
        let hash_builder = BuildHasherDefault::<DefaultHasher>::default();

        b.iter(|| {
            black_box(std_default(&address, &hash_builder));
        })
    });

    #[cfg(feature = "hashbrown")]
    group.bench_function("foldhash", |b| {
        let hash_builder = hashbrown::DefaultHashBuilder::default();

        b.iter(|| {
            black_box(hashbrown(&address, &hash_builder));
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
