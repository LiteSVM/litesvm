use {
    criterion::{criterion_group, criterion_main, Criterion},
    solana_address::Address,
    std::{
        hash::{BuildHasher, BuildHasherDefault, DefaultHasher},
        hint::black_box,
    },
};

#[inline(never)]
fn std_default(address: &Address, hash_builder: &BuildHasherDefault<DefaultHasher>) -> u64 {
    hash_builder.hash_one(address)
}

#[cfg(feature = "hashbrown")]
#[inline(never)]
fn hashbrown(address: &Address, hash_builder: &hashbrown::DefaultHashBuilder) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = hash_builder.build_hasher();
    address.hash(&mut hasher);
    hasher.finish()
}

fn criterion_benchmark(c: &mut Criterion) {
    let address = Address::new_unique();

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
