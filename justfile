update-features:
    python3 scripts/update_features.py
    cargo fmt -p litesvm

fmt:
    cargo +nightly fmt --all

clippy:
    cargo clippy --all-features --all-targets