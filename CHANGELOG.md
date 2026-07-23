# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.2](https://github.com/throgxyz/tronz/compare/v0.4.1...v0.4.2) - 2026-07-23

### Added

- *(signer)* add signMessageV2-compatible message signing

### Other

- fix examples link and tidy v0.4.1 changelog
- add gRPC codec tests, fixture replay, and coverage CI

### Added

- TronWeb `signMessageV2`-compatible personal message signing and verification —
  `TronSigner::sign_message`, `hash_message`, `recover_message_address`, and
  `verify_message` — using the TRON-prefixed message format documented as
  TIP-191-compatible. `RecoverableSignature` also gains address recovery
  (`recover_address_from_prehash`) and `27`/`28` legacy recovery-id encoding
  (`to_legacy_bytes`) for TronWeb / TronLink interoperability. `to_bytes()`
  (`0`/`1`, the native transaction encoding) is unchanged.

## [0.4.1](https://github.com/throgxyz/tronz/compare/v0.4.0...v0.4.1) - 2026-07-19

### Added

- `SolidityProvider`: a read-only provider over a TRON SolidityNode
  (`protocol.WalletSolidity`) exposing solidified — irreversible — state. It has
  no signer, fillers, or broadcast, so mutating state through it is a
  compile-time error. Connect via `SolidityProvider::connect` or the pre-connect
  `SolidityProvider::builder()`.
- `SolidityTransport`: a sealed, read-only transport trait with nine
  `WalletSolidity` RPCs (blocks, account, transaction/receipt lookups, block
  receipt lists and counts, constant calls, and energy estimation), plus a
  `SolidityGrpcTransport` gRPC implementation sharing connection/retry/API-key
  logic with the FullNode transport.
- `SolidityProvider::wait_for_transaction` / `wait_for_success` polling helpers,
  and `PendingTransaction::await_solidified` / `await_solidified_success` (and
  their `*_with` variants) to bridge from a FullNode broadcast straight to
  solidification.
- `ContractReadProvider`, a shared contract-call capability implemented by both
  FullNode providers and `SolidityProvider`. `ContractInstance`, TRC20/TRC721
  instances, `TronEventFilter`, and `tron_sol!` bindings can now read either
  latest or solidified state without gaining write capabilities.
- `.caller(address)` on contract instances and call builders for explicitly
  setting `msg.sender` during constant calls and energy estimation.
- `get_paginated_now_witness_list(offset, limit)` on `TronProvider` and
  `SolidityProvider` (java-tron 4.8.1's `GetPaginatedNowWitnessList`), returning
  SRs sorted by real-time vote count. Also added `list_witnesses` to
  `SolidityProvider` for solidified SR lookups. The local `WalletSolidity`/`Wallet`
  protobuf definitions were synced with the new RPC.
- Solidified stake/delegation queries on `SolidityProvider`, mirroring the
  FullNode `TronProvider` methods: `get_delegated_resource[_v1]`,
  `get_delegated_resource_index[_v1]`, `get_can_delegate_max`,
  `get_available_unfreeze_count`, and `get_can_withdraw_unfreeze_amount`.

### Changed

- `Log` now lives in `tronz-primitives` (`tronz_primitives::Log`) instead of
  `tronz-provider`. The `tronz_provider::types::Log` re-export has been removed;
  import it from `tronz-primitives` (or the `tronz` umbrella) instead. The type,
  its fields, and the `Log::new` constructor are otherwise unchanged.
- Read-only contract calls without a signer no longer use the contract address
  as `msg.sender`. They now fall back to the zero address (most view functions
  ignore the caller); supply `.caller(address)` when `msg.sender` matters.

### Fixed

- `get_transaction_info` now reports `TxStatus::Failed` when the contract-level
  result is a revert / out-of-energy / failure even if the top-level result flag
  is `0`, so `await_success` no longer misclassifies a reverted transaction as
  successful.

## [0.4.0] - 2026-07-16

### Added

- New `tronz-abi` crate with a protobuf-independent `TronAbi` metadata model,
  forward-compatible unknown enum values, optional serde support, and an
  optional Alloy `JsonAbi` bridge.
- `DeployBuilder::tron_abi` for deploying with native TRON ABI metadata while
  `DeployBuilder::abi` continues to accept Alloy's `JsonAbi`.

### Changed (Breaking)

- The minimum supported Rust version is now 1.91.1, matching the current tonic
  and AWS SDK dependency requirements.
- `ConstantCallResult::output` now uses reference-counted `Bytes` instead of
  `Vec<u8>` to avoid copying contract return data.
- Transaction memos now use reference-counted `Bytes` throughout
  `TransactionRequest` and every transaction builder.
- Market order IDs now use the fixed-size `B256` type in contracts, query APIs,
  builders, and returned order metadata instead of unchecked byte vectors.
- Low-level contract deployment and metadata APIs now use native `TronAbi`
  instead of JSON-encoded `Vec<u8>` values. Alloy `JsonAbi` conversion lives in
  `tronz-abi` behind its `alloy` feature.
- `ProviderBuilder::with_recommended_fillers()` no longer installs
  `TaposFiller`: every currently supported transaction is constructed by a
  node endpoint that already fills TAPOS. Explicit `.with_tapos()` remains
  available and its fields are now applied to the returned transaction.

### Performance

- Block-summary RPCs now use wire-compatible lightweight protobuf responses
  that skip transaction payloads when only block number, timestamp, and hash
  are requested.
- High-volume protobuf fields such as calldata, bytecode, log data, constant
  results, transaction memo data, and signatures now use reference-counted
  `bytes::Bytes` across the tonic boundary.
- Concurrent `TaposFiller` cache misses are coalesced into one
  `get_now_block` request.

### Fixed

- Contract deployment now includes the supplied ABI in the TRON protobuf
  request instead of silently sending `abi: None`.
- Contract ABI responses are preserved exactly as `TronAbi`, including bare
  tuples and unknown protobuf enum values, instead of being replaced with an
  empty byte array or making the entire contract query fail.

## [0.3.0] - 2026-07-10

### Added

- Exact decimal parsing via `FromStr` / `parse_trx` and fixed-precision
  formatting via `format_trx`. Their syntax and 6-decimal behavior mirror
  alloy's unit helpers while rejecting negative values and amounts above
  `i64::MAX` sun.

### Changed (Breaking)

- Removed the floating-point `Trx::from_trx(f64)` and `Trx::as_trx()` methods,
  along with `AmountError::OutOfRange`; use exact string parsing,
  `format_trx`, or raw sun access instead.
- `Trx` display output is now exact and fixed to 6 fractional digits without a
  `TRX` suffix (`1.500000` instead of the lossy `1.5 TRX`).
- `Trx` addition and subtraction now panic on overflow, negative operands, or a
  negative result instead of using signed saturating arithmetic. The checked
  variants return `None` for the same invalid cases.

## [0.2.2] - 2026-07-10

### Added

- `tron_sol!` now accepts `abigen`-style JSON ABI input: `Name, "path/to/abi.json"`
  - Both raw `[...]` arrays and Forge artifacts `{"abi":[...]}` are supported
  - Path is resolved relative to `CARGO_MANIFEST_DIR` and canonicalized
  - `include_bytes!` is emitted so rustc re-expands the macro when the file changes
  - `#![sol(...)]` inner attributes are forwarded to alloy's `sol!` correctly

## [0.2.1] - 2026-07-06

### Added

- `TaposFiller` 3-second TTL block cache — concurrent transactions share a single `get_now_block` round-trip; cache is shared across clones via `Arc`; configurable via `with_block_ttl()`
- `TransactionRequest::with_tapos(&block, expiry)` — fill TAPOS fields directly from a known `BlockInfo`, bypassing the filler with zero network overhead (intended for indexers)
- `TronProvider::get_block_by_number(num)` — fetch a block by height (was missing from the trait despite being available at the transport layer)
- `Log::new(address, topics, data)` constructor (non-exhaustive struct)

### Fixed

- `TronCallBuilder::call` now promotes empty contract output to `ContractError::ZeroData` instead of `ContractError::Abi`, consistent with the dynamic `Interface` API

### Internal

- Unit tests for `ContractError::decode_err`, `TronCallBuilder::call`, `TronEventFilter::decode_logs`, `TaposFiller`, and `FeeLimitFiller`

## [0.2.0] - 2026-06-20

### Added

- Request timeouts and automatic retries for the gRPC transport
  - `GrpcTransport::builder()` → `GrpcTransportBuilder` with `with_connect_timeout()`, `with_request_timeout()`, `with_retry()`, `with_endpoints()`, `maybe_api_key()`
  - `ProviderBuilder` gains `with_connect_timeout()`, `with_request_timeout()`, `with_retry()`, `with_endpoints()`
  - `GrpcTransportConfig` (`connect_timeout` 10 s, `request_timeout` 30 s, `retry`, `endpoints`) and `RetryConfig` (3 attempts, 500 ms initial back-off, ×2 multiplier capped at 10 s, ±25 % jitter)
  - Retries are limited to idempotent reads on `Unavailable` / `ResourceExhausted` / `Aborted`; `broadcast_transaction` is never retried
- Client-side failover / load balancing across equivalent nodes
  - `GrpcTransportConfig::endpoints` (and the `with_endpoints()` setters) join the primary URI; with two or more endpoints the channel is built via tonic's `Channel::balance_list` (lazy connect, automatic fail-over), while a single endpoint keeps the eager, fail-fast connect
- `MockTransport` for testing without a live node (`mock` feature)
  - Per-method FIFO response queues via `push_ok()` / `push_err()`; compose with `RootProvider` / `FilledProvider` to exercise real provider→transport delegation

### Changed (Breaking)

- `TronTransport` and `TronProvider` are now **sealed** — only `tronz` may implement them. Use the `mock`-feature `MockTransport` for tests instead of hand-rolled implementations.
- `GrpcTransportConfig` and `RetryConfig` are `#[non_exhaustive]`; construct them via `GrpcTransport::builder()` / `ProviderBuilder` and the `with_*` setters (future fields can then be added without further breaking changes).

## [0.1.2] - 2026-06-17

### Added

- BIP-39 mnemonic + BIP-44 HD key derivation (`signer-mnemonic` feature)
  - `MnemonicBuilder<W>` — derive a `LocalSigner` from any wordlist phrase; `.index()`, `.derivation_path()`, `.password()`, `.write_to()`
  - `MnemonicBuilder::build_random()` — generate a random phrase and return both signer and phrase string
  - `MnemonicKey` — pre-derived parent key for efficient sequential child derivation; `IntoIterator` support
  - Default path `m/44'/195'/0'/0/0` (TRON BIP-44 coin type 195)
  - Test vector cross-verified against gotron-sdk (`abandon`×11 + `about`, index 0)
- Web3 Secret Storage V3 keystore (`signer-keystore` feature)
  - `LocalSigner::encrypt_keystore(dir, password)` — scrypt (N=2¹⁸, r=8, p=1) + AES-128-CTR + keccak256 MAC
  - `LocalSigner::decrypt_keystore(path, password)` — verifies MAC before decrypting
  - `KeystoreFile` struct exposed for inspection of JSON fields (version, id, kdf params)
  - Address stored as TRON base58check; compatible with TronLink and gotron-sdk
- Stake 1.0 legacy support
  - `FreezeBalanceV1Contract` / `UnfreezeBalanceV1Contract` domain types
  - `provider.freeze_balance_v1()` → `FreezeV1Builder` (`.amount()`, `.resource()`, `.frozen_duration()`, `.receiver()` for inline delegation)
  - `provider.unfreeze_balance_v1()` → `UnfreezeV1Builder` (releases all staked TRX immediately, no unbonding delay)
  - `provider.get_delegated_resource_v1()` / `get_delegated_resource_index_v1()` — query Stake 1.0 delegation state
  - New example: `stake_v1`

### Changed (Breaking)

- All transaction builders: `.owner(Address)` renamed to `.from(Address)` — aligns with alloy's `TransactionRequest` convention
- `WitnessApi::update_witness_url()` renamed to `update_witness()` — consistent with `update_brokerage()` and `become_witness()`
- `UndelegateBuilder::from(delegatee)` renamed to `receiver(delegatee)` — avoids collision with the new `.from()` sender override and aligns with the proto field name `receiver_address`

## [0.1.1] - 2026-06-16

### Added

- `WitnessApi` extension trait (`use tronz::providers::ext::WitnessApi as _`)
  - `list_witnesses()`, `get_brokerage()`, `get_reward_info()`
  - `become_witness()`, `update_witness_url()`, `update_brokerage()`
- `GovernanceApi` extension trait (`use tronz::providers::ext::GovernanceApi as _`)
  - `list_proposals()`, `get_paginated_proposal_list()`, `get_proposal_by_id()`
  - `submit_proposal()`, `approve_proposal()`, `cancel_proposal()`
  - `ProposalInfo` and `ProposalState` domain types
- Full `Trc10Api` extension trait (`use tronz::providers::ext::Trc10Api as _`)
  - `participate_trc10()` — buy TRC10 tokens in an ICO
  - `unfreeze_trc10()` — release locked TRC10 supply after the lock period
  - `update_trc10()` — update token description, URL, and bandwidth limits
  - `get_asset_issue_by_name()` / `get_asset_issue_list_by_name()` — look up tokens by name
- Contract management builders
  - `set_account_id()` — set a unique alphanumeric on-chain account alias
  - `clear_contract_abi()` — wipe a deployed contract's ABI
  - `update_contract_setting()` — update the caller-energy percentage
  - `update_contract_energy_limit()` — update the per-call origin energy cap
- Network domain types: `NodeInfo`, `NodeAddress`, `ChainProperties`, `SignWeight`, `AccountNet`
- Extensive `TronTransport` / `TronProvider` coverage
  - Chain: `get_chain_parameters()`, `get_dynamic_properties()`, `get_node_info()`, `list_nodes()`
  - Pricing: `get_bandwidth_prices()`, `get_energy_prices()`, `get_memo_fee()`
  - Timing: `get_next_maintenance_time()`, `get_burn_trx()`, `get_total_transactions()`
  - Block: `get_block_by_id()`, `get_blocks_by_latest_num()`, `get_blocks_by_limit()`, `get_transaction_count_by_block_num()`
  - Tx history: `get_transactions_from()`, `get_transactions_to()`, `get_transaction_info_by_block_num()`
  - Pending pool: `get_pending_size()`, `get_transaction_from_pending()`, `get_pending_transactions()`
  - Multi-sig: `get_transaction_sign_weight()`, `get_transaction_approved_list()`
  - Staking: `get_can_withdraw_unfreeze_amount()`, `get_available_unfreeze_count()`
  - Account: `get_account_net()`
- Examples: `governance_list`, `trc10_by_name`
- `Makefile` with local CI targets: `make ci`, `make test`, `make clippy`, `make fmt`, `make docs`, `make typos`, `make deny`, `make features`

### Fixed

- Pinned GitHub Actions to exact commit SHAs
- Added `typos.toml` to exclude `proto/` and suppress crypto-related false positives

## [0.1.0] - 2026-06-14

### Added

#### Core infrastructure
- `tronz-primitives` crate: `Address` (base58check + hex), `Trx` (sun-denominated), `ResourceCode`, `RecoverableSignature`
- `tronz-signer` crate: `TronSigner` trait, `LocalSigner` (in-memory secp256k1 from hex private key)
- `tronz-provider` crate: gRPC transport via `tonic` targeting TronGrid (mainnet + Nile testnet)
- `tronz-contract` crate: TRC20 / smart-contract bindings
- `tronz` umbrella crate re-exporting all public APIs
- `ProviderBuilder` with composable filler chain: `TaposFiller`, `FeeLimitFiller`, `SignerFiller`
- `PendingTransaction` polling (3 s interval, 20 attempts, 60 s timeout)

#### Native contract builders (on `TronProvider`)
- TRX transfer — `send_trx()`
- Stake 2.0 — `freeze_balance()`, `unfreeze_balance()`, `delegate_resource()`, `undelegate_resource()`, `withdraw_expire_unfreeze()`, `cancel_all_unfreeze()`
- Rewards — `claim_rewards()`
- Voting — `vote_witness()`
- Account management — `create_account()`, `update_account_name()`, `update_permissions()`

#### TRC10 (`Trc10Api` extension trait)
- `issue_trc10()` — issue a new TRC10 native token
- `transfer_trc10()` — transfer TRC10 tokens
- `get_trc10_balance()`, `get_asset_issue_by_id()`, `get_asset_issue_list()`, `get_asset_issue_by_account()`

#### TRC20 / smart contracts
- `Trc20Instance<P>` — typed bindings: `name`, `symbol`, `decimals`, `total_supply`, `balance_of`, `transfer`, `approve`, `transfer_from`, `allowance`
- `ContractInstance` + `CallBuilder` + `DeployBuilder` for dynamic ABI interaction via `Interface` / `alloy-json-abi`
- Event decoding helpers: `decode_logs`, `decode_log`
- Energy estimation: `estimate_energy()`

#### Examples (38, Nile testnet)
- Read-only: `query`, `address_formats`, `amount_math`, `connect_custom`, `signer_generate`, `signer_local`, `list_witnesses`, `trc10_query`, `trc10_balance`, `trc20_decode_transfer_event`, `decode_log`, `decode_receipt`, `contract_call`, `contract_estimate_energy`, `contract_revert`
- With private key: `transfer_trx`, `transfer_trx_memo`, `stake`, `stake_bandwidth`, `delegate`, `undelegate`, `unfreeze`, `cancel_unfreeze`, `withdraw_unfreeze`, `claim_rewards`, `vote_witness`, `trc10_transfer`, `trc10_issue`, `account_create`, `account_update`, `account_permissions`, `trc20`, `trc20_approve`, `trc20_transfer_from`, `contract_send`, `contract_deploy`, `contract_dynamic_abi`

#### CI / tooling
- GitHub Actions: test matrix (ubuntu + windows, stable + nightly + MSRV 1.85), clippy, fmt, docs, typos, cargo-deny, feature-powerset check, CodeQL
- `.config/nextest.toml`, `deny.toml`, `cliff.toml`
- GitHub issue templates, PR template, Dependabot config
