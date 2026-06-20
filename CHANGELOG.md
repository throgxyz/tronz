# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
