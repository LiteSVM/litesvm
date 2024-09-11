# Changelog

## [Unreleased]

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

[Unreleased]: https://github.com/LiteSVM/litesvm/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/LiteSVM/litesvm/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/LiteSVM/litesvm/releases/tag/v0.1.0
