# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.2](https://github.com/throgxyz/tronz/compare/v0.5.1...v0.5.2) - 2026-08-11

### Other

- Add more description for catfee
- update Cargo.toml dependencies

## [0.5.1](https://github.com/throgxyz/tronz/compare/v0.5.0...v0.5.1) - 2026-08-06

### Added

- *(rpc-types)* expand transaction, receipt, block, and permission types

### Other

- update Cargo.toml dependencies

### Added

- Added richer block, transaction, receipt, permission, and internal-call data.
- Added raw and signed transaction inspection, decoding, and wire encoding.

### Changed

- Active permissions now require an explicit `OperationSet`.
- Receipt energy fields and `ContractResult` now preserve their precise node semantics.
- Transaction lookup now rejects malformed signatures and mismatched transaction IDs.

## [0.5.0](https://github.com/throgxyz/tronz/compare/v0.4.1...v0.5.0) - 2026-07-30

### Added

- Added `tronz-rpc-types`, a network-independent crate for TRON domain types,
  protobuf messages, and codecs. `tronz-provider::types` remains a compatible
  re-export, and committed generated code removes `protoc` from normal builds.
- Added multi-key `TronWallet` support, strict owner-key routing, multisig
  permissions, unsigned transaction builds, and multi-signer helpers.
- Added composable provider layers, `DynProvider`, custom fillers, gRPC
  middleware, and named `ReadProvider`/`WalletProvider` stacks.
- Added `EnergyFiller`, which estimates contract energy and applies the current
  chain price, configurable margin, bounds, cache, and failure policy.
- Added configurable pending-transaction polling, receipt success checks, and
  solidified receipt support.
- Added TronWeb-compatible message signing, TIP-712 typed data, synchronous
  signer support, and PBKDF2 keystore decryption.
- Added provider-bound `tron_sol!` improvements, Forge artifact deployment,
  event ranges and watchers, anonymous events, and typed TRC20/TRC721 call
  builders.
- Added `BlockInfo::new`, contract fee-limit controls, ABI lookup helpers, and
  additional primitive conversions.

### Changed (Breaking)

- `TransportErrorKind::Grpc` is replaced by transport-neutral
  `Rpc { code: RpcStatusCode, message }`; `Connect` now boxes its source error.
  Use `is_rpc()` and `status_code()` instead of `is_grpc()` and `tonic::Code`.
- Recommended fillers now use dynamic `EnergyFiller` pricing instead of a flat
  fee limit. Use `with_fee_limit` to keep a fixed limit.
- Providers erase their transport inside `RootProvider`. `TronProvider` drops
  its transport associated type, is object-safe and unsealed, and requires only
  `root()`. `FilledProvider<T, F>` becomes `FilledProvider<F>`.
- `TronTransport` and `SolidityTransport` are object-safe and no longer require
  `Clone` or an error associated type.
- `SignerFiller` is replaced by `WalletFiller`; `TronSigner::sign_hash` now takes
  `&B256`, and provider signing uses `TronWallet`.
- Transaction and block lookups return `Option` when the requested value is not
  present.
- `TxFiller::status` and `FillerStatus` are removed.
- Contract `caller` also selects the sender for writes; deployments use `from`.
- `send_transaction` rejects requests without an owner.
- `Log::topics` is private and `Log::new` validates the four-topic limit.
- Keystore KDF parameters now use `KdfparamsType`;
  `tronz::signer_aws` moves to `tronz::signers::aws`; and
  `TronAbiConversionError` requires `tronz-abi/alloy`.
- `ContractType` is exhaustive so adding an operation without a transport route
  fails at compile time.

### Fixed

- Verify node-built transactions, contracts, permission IDs, and locally
  computed transaction IDs before signing.
- Distinguish missing transactions and blocks from malformed node responses.
- Preserve the transaction ID when a broadcast outcome is ambiguous and never
  retry a broadcast automatically.
- Make pending timeouts cover total wall-clock time and continue polling through
  transient node failures while preserving the last error.
- Fixed keystore validation, Forge bytecode deployment, contract fee limits,
  dynamic contract sends, typed event decode errors, anonymous events, and the
  TRC721 four-argument `safeTransferFrom` overload.

### Changed

- `ProviderBuilder::new()` installs recommended fillers; `default()` installs
  none. Custom transports connect with `connect_transport`.
- `on_grpc*` is deprecated in favor of `connect_grpc*`. Pending transaction
  `await_*` aliases are deprecated in favor of receipt-oriented names.
- The facade `full` feature now includes mnemonic, keystore, and TIP-712 support.

## [0.4.1](https://github.com/throgxyz/tronz/compare/v0.4.0...v0.4.1) - 2026-07-19

### Added

- Added `SolidityProvider` and `SolidityGrpcTransport` for read-only access to
  irreversible SolidityNode state, including blocks, accounts, transactions,
  receipts, contract calls, energy estimates, witnesses, and staking data.
- Added `ContractReadProvider`, allowing contract instances, generated bindings,
  and event filters to read from either FullNode or SolidityNode providers.
- Added solidification polling and success checks, including bridging a
  FullNode broadcast directly to a solidified receipt.
- Added explicit contract-call callers and paginated witness queries.

### Changed (Breaking)

- `Log` moves from `tronz-provider` to `tronz-primitives`; import it from
  `tronz_primitives` or the `tronz` facade.
- Read-only calls without an explicit caller now use the zero address instead
  of the contract address.

### Fixed

- Receipt decoding now treats contract-level revert, out-of-energy, and failure
  results as failed even when the top-level result flag is unset.

## [0.4.0](https://github.com/throgxyz/tronz/compare/v0.3.0...v0.4.0) - 2026-07-16

### Added

- Added `tronz-abi`, a protobuf-independent TRON ABI model with optional serde
  and Alloy `JsonAbi` conversion.
- Added native `TronAbi` contract deployment and metadata support.

### Changed (Breaking)

- Raised the minimum supported Rust version to 1.91.1.
- Contract output and transaction memo data now use reference-counted `Bytes`.
- Market order IDs now use `B256`; contract ABI APIs now use `TronAbi`.
- Recommended fillers no longer install `TaposFiller`, because supported
  transactions are constructed by node endpoints that already provide TAPOS.

### Performance

- Block summary RPCs avoid decoding transaction payloads when only block
  metadata is required.
- High-volume protobuf byte fields use reference-counted buffers.
- Concurrent TAPOS cache misses are coalesced into one node request.

### Fixed

- Contract deployment now sends the supplied ABI, and contract ABI responses
  preserve tuples and unknown enum values.

## [0.3.0](https://github.com/throgxyz/tronz/compare/v0.2.2...v0.3.0) - 2026-07-10

### Added

- Added exact decimal TRX parsing and fixed-precision formatting.

### Changed (Breaking)

- Removed floating-point TRX conversion; use string parsing, formatting, or raw
  sun values.
- `Trx` display is exact with six fractional digits and no `TRX` suffix.
- Invalid or overflowing TRX arithmetic now panics; checked variants return
  `None`.

## [0.2.2](https://github.com/throgxyz/tronz/compare/v0.2.1...v0.2.2) - 2026-07-10

### Added

- `tron_sol!` now accepts inline JSON ABI, ABI files, and Forge artifacts, with
  Cargo-aware path tracking and Alloy attribute passthrough.

## [0.2.1](https://github.com/throgxyz/tronz/compare/v0.2.0...v0.2.1) - 2026-07-06

### Added

- Added a shared TTL cache for `TaposFiller`, direct TAPOS filling from a known
  block, block lookup by height, and `Log::new`.

### Fixed

- Empty typed contract-call output now reports `ContractError::ZeroData`.

## [0.2.0](https://github.com/throgxyz/tronz/compare/v0.1.2...v0.2.0) - 2026-06-20

### Added

- Added configurable gRPC connection/request timeouts, retries, exponential
  backoff, endpoint failover, and load balancing.
- Added `MockTransport` for deterministic provider tests.

### Changed (Breaking)

- `TronTransport` and `TronProvider` became sealed; use `MockTransport` instead
  of downstream transport implementations.
- `GrpcTransportConfig` and `RetryConfig` became non-exhaustive and are
  configured through builders.

## [0.1.2](https://github.com/throgxyz/tronz/compare/v0.1.1...v0.1.2) - 2026-06-17

### Added

- Added BIP-39/BIP-44 mnemonic and HD wallet derivation.
- Added Web3 Secret Storage V3 keystore encryption and decryption.
- Added Stake 1.0 freeze, unfreeze, and delegation support.

### Changed (Breaking)

- Transaction builder `owner` methods were renamed to `from`.
- Witness URL and undelegation builder methods were renamed for consistency
  with their domain fields.

## [0.1.1](https://github.com/throgxyz/tronz/compare/v0.1.0...v0.1.1) - 2026-06-16

### Added

- Added witness, governance, TRC10, contract-management, network, pricing,
  pending-pool, multisig, staking, and account query APIs.
- Added reusable local CI, dependency, security, and feature-matrix tooling.

### Fixed

- Pinned GitHub Actions and added typo exclusions for generated protocol files.

## [0.1.0] - 2026-06-14

### Added

- Initial release of the `tronz` facade and the primitives, signer, provider,
  and contract crates.
- Added TRON addresses and amounts, local signing, gRPC providers, composable
  fillers, pending transaction polling, and native transaction builders.
- Added Stake 2.0, voting, rewards, account management, TRC10, TRC20, dynamic
  contracts, deployment, calls, energy estimation, and event decoding.
- Added Nile examples and cross-platform CI, lint, documentation, security, and
  dependency checks.
