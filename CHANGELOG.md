# Changelog

## [Unreleased]

### Changed

- Upgrade Solana crates to 2.2.0 ([#138](https://github.com/LiteSVM/litesvm/pull/138)).

## [0.5.0] - 2025-01-23

### Added

- Add PartialEq for some types ([#126](https://github.com/LiteSVM/litesvm/pull/126)).

### Changed

- Make the LiteSVM struct thread-safe ([#127](https://github.com/LiteSVM/litesvm/pull/127)).

### Fixed

- Fix Solana dependencies ([#119](https://github.com/LiteSVM/litesvm/pull/119)).

## [0.4.0] - 2024-12-30

### Changed

- Bump Solana crates to 2.1 ([#96](https://github.com/LiteSVM/litesvm/pull/96)).

### Added

- Add `LiteSVM::with_precompiles` ([#102](https://github.com/LiteSVM/litesvm/pull/102)).

### Fixed

- Fix account executable in the `add_builtin` method ([#110](https://github.com/LiteSVM/litesvm/pull/110)).

## [0.3.0] - 2024-10-12

### Added

- Make log_bytes_limit configurable ([#96](https://github.com/LiteSVM/litesvm/pull/96)).

### Changed

- Include `post_accounts` in `simulate_transaction` output ([#97](https://github.com/LiteSVM/litesvm/pull/97)).

## [0.2.1] - 2024-09-27

### Changed

- Change `owner` from Keypair to Pubkey in `create_ata` and `create_ata_idempotent` helpers ([#90](https://github.com/LiteSVM/litesvm/pull/90)).

## [0.2.0] - 2024-09-11

### Added

- Add helpers for token ([#73](https://github.com/LiteSVM/litesvm/pull/73)).
- Add helpers for bpf_loader ([#73](https://github.com/LiteSVM/litesvm/pull/73)).
- Add stake, config and vote programs ([#57](https://github.com/LiteSVM/litesvm/pull/57)).
- Implement blockhash and durable nonce checks ([#61](https://github.com/LiteSVM/litesvm/pull/61)).
- Add `error.rs` and new `LiteSVMError` type ([#62](https://github.com/LiteSVM/litesvm/pull/62)).
- Add more logging for users to make debugging errors easier ([#62](https://github.com/LiteSVM/litesvm/pull/62)).
- Add `inner_instructions` to `TransactionMetadata` ([#75](https://github.com/LiteSVM/litesvm/pull/75)).
- Add feature-flagged `serde` traits to `TransactionMetadata` ([#77](https://github.com/LiteSVM/litesvm/pull/77)).

### Changed

- Accept both legacy and versioned tx in `simulate_transaction` ([#58](https://github.com/LiteSVM/litesvm/pull/58)).
- Move `InvalidSysvarDataError` to `error.rs` ([#62](https://github.com/LiteSVM/litesvm/pull/62)).
- Change `set_account` to return `Result<(), LiteSVMError>` ([#62](https://github.com/LiteSVM/litesvm/pull/62)).
- Replace `&mut self` with `&self` in `simulate_transaction`. ([#64](https://github.com/LiteSVM/litesvm/pull/64)).
- Remove `set_compute_budget` as it duplicates `with_compute_budget`. ([#68](https://github.com/LiteSVM/litesvm/pull/68)).
- Remove `set_upgrade_authority` and `deploy_upgradeable_program` ([#69](https://github.com/LiteSVM/litesvm/pull/69)).
- Change `with_builtins` to take a feature_set argument `Option<FeatureSet>` ([#81](https://github.com/LiteSVM/litesvm/pull/81)).

## [0.1.0] - 2024-04-02

### Added

- Initial release.

[Unreleased]: https://github.com/LiteSVM/litesvm/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/LiteSVM/litesvm/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/LiteSVM/litesvm/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/LiteSVM/litesvm/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/LiteSVM/litesvm/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/LiteSVM/litesvm/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/LiteSVM/litesvm/releases/tag/v0.1.0
