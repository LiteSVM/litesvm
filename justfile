default:
    @just --list

update-features:
    python3 scripts/update_features.py
    just fmt

fmt:
    cargo +nightly fmt --all

clippy:
    cargo clippy --all-features --all-targets
    
publish:
    cargo publish -p litesvm
    cargo publish -p litesvm-loader
    cargo publish -p litesvm-token
    cargo publish -p litesvm-persistence
    cargo publish -p litesvm-cpi-tree

bench:
    cd crates/litesvm/test_programs && cargo build-sbf --tools-version v1.53
    RUST_LOG= cargo bench -p litesvm

# If "perf record" is slow: https://github.com/flamegraph-rs/flamegraph/issues/74#issuecomment-1909417039
flamegraph:
    CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --bench max_perf -- --bench
