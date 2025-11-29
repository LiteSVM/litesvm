# LiteSVM (NodeJS) Changelog

## [0.4.0] - 2025-11-29

### Added

- Add aarch64-unknown-linux-gnu as a node-litesvm target [(#251)](https://github.com/LiteSVM/litesvm/pull/251)

### Changed

- Update Solana dependencies to 3.0 [(#223)](https://github.com/LiteSVM/litesvm/pull/223)
- Put precompiles behind feature flag [(#232)](https://github.com/LiteSVM/litesvm/pull/232)

### Fixed

- Fix dependencies [(#247)](https://github.com/LiteSVM/litesvm/pull/247)

## [0.3.3] - 2025-08-31

### Fixed

- Release fixes

## [0.3.2] - 2025-08-31

### Fixed

- Fix yarn js build command [(#210)](https://github.com/LiteSVM/litesvm/pull/210)

## [0.3.1] - 2025-08-28

### Fixed

- Fix npm release [(#209)](https://github.com/LiteSVM/litesvm/pull/209)

## [0.3.0] - 2025-08-28

### Added

- Add `prettyLogs` to node TransactionMetadata [(#147)](https://github.com/LiteSVM/litesvm/pull/147)

### Changed

- Update Solana to 2.3 [(#194)](https://github.com/LiteSVM/litesvm/pull/194)
- Refactor function signatures for node-litesvm
- Consolidate feature set management [(#142)](https://github.com/LiteSVM/litesvm/pull/142)
- Bump JS dependencies [(#184)](https://github.com/LiteSVM/litesvm/pull/184)
- Refactor `add_program` methods to accept program_id as `impl Into<Pubkey>` for improved flexibility [(#183)](https://github.com/LiteSVM/litesvm/pull/183)
- Make `add_program` return an error if the program is invalid [(#187)](https://github.com/LiteSVM/litesvm/pull/187)
- Update dependencies [(#182)](https://github.com/LiteSVM/litesvm/pull/182)

### Fixed

- Fix the documentation for Node [(#191)](https://github.com/LiteSVM/litesvm/pull/191)

## [0.2.0] - 2025-02-20

### Changed

- Upgraded Solana deps to 2.2 [(#138)](https://github.com/LiteSVM/litesvm/pull/138)

### Added

- Added missing functionality to `EpochSchedule` [(#123)](https://github.com/LiteSVM/litesvm/pull/123)

### Fixed

- Fixed skipping sigverify [(#135)](https://github.com/LiteSVM/litesvm/pull/135)

## [0.1.0] - 2025-01-21

First release!
