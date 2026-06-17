# tronz — Claude Code Context

## Project Overview

**tronz** is an async-first Rust SDK for the TRON blockchain, inspired by [alloy](https://github.com/alloy-rs/alloy). It provides ergonomic APIs for TRON native operations (TRX transfer, Stake 2.0, delegation) and EVM-compatible smart contracts (TRC20/TRC721).

- **Status:** v0.1.1 — active development, not yet production-ready
- **Rust edition:** 2024, MSRV 1.85 (required for stable RPITIT)
- **License:** MIT OR Apache-2.0
- **Repo:** https://github.com/throgxyz/tronz

---

## Workspace Layout

```
tronz/
├── Cargo.toml                  # Workspace root (members: crates/*)
├── Makefile                    # Local CI targets (make ci, make clippy, etc.)
├── deny.toml                   # cargo-deny config (licenses, advisories, bans)
├── typos.toml                  # typos config (excludes proto/, custom word list)
├── DESIGN.md                   # Architecture design doc (1600+ lines)
└── crates/
    ├── primitives/             # tronz-primitives: leaf types, no I/O
    ├── signer/                 # tronz-signer: TronSigner trait + LocalSigner
    ├── provider/               # tronz-provider: transport + domain model + provider
    ├── contract/               # tronz-contract: TRC20/TRC721 ABI bindings
    └── tronz/                  # umbrella crate (re-exports everything)
```

Examples live in a separate repo: https://github.com/throgxyz/examples

### Crate dependency graph

```
tronz-primitives  (leaf: Address, Trx, ResourceCode, RecoverableSignature)
      ↑        ↑
tronz-signer   tronz-provider  (domain types + proto codec + gRPC transport + provider)
                    ↑
              tronz-contract    (sol! bindings, TRC20/TRC721, call/deploy builders)
                    ↑
                  tronz         (umbrella re-export)
```

---

## Key Design Decisions

### 1. RPITIT (Return-Position Impl Trait in Traits)
All async traits (`TronSigner`, `TronTransport`, `TronProvider`) use stable RPITIT instead of `async_trait`. This gives zero-cost futures (no `Box<dyn Future>` heap allocation). Requires Rust 1.85+.

### 2. gRPC Transport (not JSON-RPC)
TRON nodes expose a gRPC `WalletClient` service. tronz uses `tonic` + `prost` directly. There is no JSON-RPC equivalent for TRON (unlike Ethereum).

**Default endpoints:**
- Mainnet: `https://grpc.trongrid.io:443` (`TRONGRID_MAINNET`)
- Nile testnet: `http://grpc.nile.trongrid.io:50051` (`TRONGRID_NILE`)

### 3. Proto Types Are Private
Prost-generated code lives in `provider/src/proto/` (private module). All public APIs use domain types (`RawTransaction`, `AccountInfo`, `BlockInfo`). Codec conversions are hidden inside `transport/grpc/codec.rs`.

### 4. Filler Chain (Adapted from Alloy)
```
provider.send_trx().to(addr).amount(trx).send()
  → TaposFiller: fetch latest block, fill ref_block_*, expiration, timestamp
  → FeeLimitFiller: set fee_limit for smart contract calls
  → SignerFiller: sign tx_id with LocalSigner
  → broadcast_transaction()
  → PendingTransaction { tx_id }
```
Fillers are composed via `JoinFill<L, R>`. `ProviderBuilder::with_recommended_fillers()` chains `TaposFiller + FeeLimitFiller`.

### 5. Two Transaction Build Paths
- **Native contracts** (transfer, freeze, delegate…): Client builds `RawTransaction` locally, TaposFiller fills TAPOS fields, client signs and broadcasts.
- **Smart contracts** (`TriggerSmartContract`, `CreateSmartContract`): Client sends params to node → node builds `RawTransaction` → client signs locally → broadcasts. TaposFiller skips if `ref_block_bytes` is already set.

### 6. Extension Trait Pattern (Trc10Api, etc.)
Extra functionality that doesn't belong on `TronProvider` directly lives in extension traits under `provider/src/ext/`. Import the trait to unlock the methods:
```rust
use tronz::providers::ext::Trc10Api as _;
provider.issue_trc10().name("MTK").send().await?;
```

### 7. Alloy Integration Strategy
| What | How |
|------|-----|
| Primitive types (B256, U256, Bytes) | Direct dep on `alloy-primitives` |
| TRC20/TRC721 ABI codec | `alloy-sol-macro` (`sol!`) + `alloy-sol-types` |
| Dynamic ABI | `alloy-dyn-abi` + `alloy-json-abi` |
| Provider/Transport/Network traits | NOT reused (TRON-specific) |
| ProviderBuilder/TxFiller/JoinFill | Adapted pattern, not the same code |
| Module visibility | Private `mod` + `pub use` re-exports (same as alloy-contract) |

### 8. TRON Address Format
```
TRON address = 0x41 || EVM-body (20 bytes) = 21 bytes total
Base58check:  "T..." (user-facing)
Hex:          "41..." or "0x41..."

FromStr auto-detects: starts with 'T' → base58, else → hex
```

---

## Module Map (tronz-provider internals)

```
provider/src/
├── types/          # Public domain model (no proto leakage)
│   ├── transaction.rs    # RawTransaction, SignedTransaction, TransactionRequest
│   ├── block.rs          # BlockInfo + TAPOS extraction helpers
│   ├── account.rs        # AccountInfo, AccountResource, DelegatedResource
│   ├── contract.rs       # ContractType enum + param structs (incl. AssetIssueContract)
│   ├── receipt.rs        # TransactionInfo, Log, ContractResult
│   └── trc10.rs          # AssetInfo
├── transport/
│   ├── mod.rs            # TronTransport trait
│   └── grpc/
│       ├── mod.rs        # GrpcTransport (tonic WalletClient)
│       └── codec.rs      # Proto ↔ domain type conversions
├── provider/
│   ├── mod.rs            # TronProvider trait
│   ├── root.rs           # RootProvider<T> (Arc-backed, cheap clone)
│   ├── builder.rs        # ProviderBuilder, FilledProvider<T,F>, JoinFill<L,R>
│   └── pending.rs        # PendingTransaction (polls get_transaction_info, 3s × 20)
├── fillers/
│   └── mod.rs            # TaposFiller, FeeLimitFiller, SignerFiller, Identity
├── builders/
│   ├── transfer.rs       # TransferBuilder
│   ├── freeze.rs         # FreezeBuilder / UnfreezeBuilder
│   ├── delegate.rs       # DelegateBuilder / UndelegateBuilder
│   ├── withdraw.rs       # WithdrawExpireBuilder / CancelAllUnfreezeBuilder
│   ├── rewards.rs        # WithdrawBalanceBuilder (claim block/vote rewards)
│   ├── permission.rs     # AccountPermissionUpdateBuilder
│   ├── vote.rs           # VoteBuilder
│   └── account.rs        # CreateAccountBuilder / UpdateAccountBuilder
└── ext/
    ├── trc10.rs          # Trc10Api: transfer_trc10, issue_trc10, get_asset_info, …
    ├── witness.rs        # WitnessApi: list_witnesses, brokerage, update_witness, …
    └── governance.rs     # GovernanceApi: list_proposals, get_proposal_by_id, submit, approve, cancel
```

---

## Tech Stack

| Layer | Crate | Note |
|-------|-------|------|
| Async | tokio 1 | rt-multi-thread |
| gRPC | tonic 0.14 + prost 0.14 | tls-ring feature |
| Crypto | k256 0.13 | secp256k1 ECDSA |
| Hashing | sha2 0.10 | SHA-256 for tx_id |
| Address encoding | bs58 0.5 | base58check |
| ABI codec | alloy-sol-macro + alloy-sol-types 1.x | TRC20/TRC721 |
| Dynamic ABI | alloy-dyn-abi + alloy-json-abi 1.x | JSON ABI contracts |
| Error | thiserror 2 | one enum per crate |
| Tracing | tracing 0.1 | optional instrumentation |

---

## Feature Flags

```toml
# tronz umbrella
default = ["provider-grpc-tls", "contract", "signer-local"]
full    = ["default"]
provider-grpc-tls = ["tronz-provider/grpc-tls"]   # TLS (production)
provider-grpc     = ["tronz-provider/grpc"]         # No TLS (local nodes)
contract          = ["dep:tronz-contract"]
signer-local      = []  # doc flag; LocalSigner always compiled

# tronz-provider
default  = ["grpc-tls"]
grpc-tls = ["tonic/tls-native-roots"]
grpc     = []
```

---

## What's Implemented (v0.1.0)

- [x] Primitives: Address, Trx, ResourceCode, RecoverableSignature
- [x] LocalSigner (in-memory secp256k1 from hex private key)
- [x] gRPC transport with TLS (TronGrid mainnet + Nile testnet)
- [x] TronProvider with filler chain (TAPOS + fee limit + signer)
- [x] Builders: transfer, freeze, unfreeze, delegate, undelegate, withdraw, cancel-unfreeze, claim rewards, vote, account create/update, account permission update
- [x] Queries: account info, resources, delegation index, max delegatable, pending reward, block, witnesses
- [x] TRC10: transfer, issue, balance query, asset info, asset list (`Trc10Api` extension trait)
- [x] TRC20 bindings via `sol!` + `Trc20Instance<P>` (name, symbol, decimals, totalSupply, balanceOf, transfer, approve, transferFrom)
- [x] Dynamic ABI: `Interface` + `ContractInstance` (call/send/deploy by name)
- [x] PendingTransaction polling (3s interval, 20 attempts = 60s timeout)
- [x] Event log decoding helpers (`decode_logs`, `decode_log`)
- [x] ContractInstance / CallBuilder / DeployBuilder
- [x] 42 examples, all tested on Nile testnet (see https://github.com/throgxyz/examples)
- [x] CI pipeline: test (ubuntu+windows, stable+nightly+MSRV), clippy, fmt, docs, typos, deny, feature-checks, codeql

## Not Yet Implemented

- [ ] TRC721 bindings
- [ ] Hardware wallet signers (Ledger/Trezor)
- [x] BIP39/BIP44 mnemonic derivation (`MnemonicBuilder`, `signer-mnemonic` feature)
- [x] Web3 Secret Storage V3 keystore (`signer-keystore` feature)
- [x] Stake 1.0 legacy support (`freeze_balance_v1`, `unfreeze_balance_v1`)
- [ ] Multi-sig signing flow
- [ ] WebSocket / pubsub streaming
- [ ] HTTP JSON API fallback
- [ ] DEX contract APIs

---

## Examples (42 total, all on Nile testnet)

| Example | Key needed | What it shows |
|---------|-----------|---------------|
| `query` | no | account info, block, resources |
| `address_formats` | no | base58 / hex / bytes conversions |
| `amount_math` | no | Trx arithmetic |
| `connect_custom` | no | custom gRPC endpoint |
| `signer_generate` | no | generate random keypair |
| `signer_local` | no | sign + verify a hash |
| `signer_mnemonic` | no | BIP-39 mnemonic derive + random generate (`signer-mnemonic`) |
| `signer_keystore` | no | encrypt / decrypt keystore file (`signer-keystore`) |
| `list_witnesses` | no | SR list sorted by votes |
| `trc10_query` | no | fetch TRC10 token metadata by ID |
| `trc10_by_name` | no | fetch TRC10 token metadata by name |
| `trc10_balance` | no | check TRC10 balance |
| `governance_list` | no | list proposals, fetch by ID (GovernanceApi) |
| `trc20` | yes | TRC20 balance + transfer |
| `trc20_approve` | yes | approve + allowance |
| `trc20_transfer_from` | yes | transferFrom flow |
| `trc20_decode_transfer_event` | no | decode Transfer logs |
| `decode_log` | no | generic event decode |
| `decode_receipt` | no | full receipt decode |
| `transfer_trx` | yes | send TRX + poll confirmation |
| `transfer_trx_memo` | yes | TRX transfer with memo |
| `stake` | yes | freeze + delegate + claim (Stake 2.0) |
| `stake_v1` | yes | freeze + unfreeze (Stake 1.0 legacy) |
| `stake_bandwidth` | yes | freeze for bandwidth |
| `delegate` | yes | delegate resource |
| `undelegate` | yes | undelegate resource |
| `unfreeze` | yes | unfreeze V2 |
| `cancel_unfreeze` | yes | cancel pending unfreeze |
| `withdraw_unfreeze` | yes | withdraw expired unfreeze |
| `claim_rewards` | yes | claim block/vote rewards |
| `vote_witness` | yes | vote for SR |
| `trc10_transfer` | yes | transfer TRC10 tokens |
| `trc10_issue` | yes | issue (create) new TRC10 token |
| `account_create` | yes | create account on-chain |
| `account_update` | yes | set account name |
| `account_permissions` | yes | multi-sig permission update |
| `contract_call` | no | read-only contract call |
| `contract_send` | yes | state-changing contract call |
| `contract_deploy` | yes | deploy a contract |
| `contract_dynamic_abi` | yes | call contract via JSON ABI |
| `contract_estimate_energy` | no | estimate energy cost |
| `contract_revert` | no | handle revert errors |

---

## Local CI Checks

Run these before every push. The `make ci` target runs all of them in order.

```bash
make ci          # run everything below in sequence
make fmt         # cargo +nightly fmt --all --check
make clippy      # RUSTFLAGS=-Dwarnings cargo clippy --all-targets --all-features
make test        # cargo nextest run --workspace
make doctest     # cargo test --workspace --doc (both default and --all-features)
make docs        # cargo +nightly doc (with full docsrs flags)
make typos       # typos
make deny        # cargo deny check
make features    # cargo hack check --feature-powerset --depth 1
```

**Required tools** (install once):

```bash
cargo install cargo-nextest typos-cli cargo-deny cargo-hack
rustup toolchain install nightly
```

**typos config:** `typos.toml` in repo root. `typos.toml` takes precedence over `_typos.toml` — do not create both. Proto files are excluded via `extend-exclude = ["crates/provider/proto/**"]`.

---

## Running Examples

Examples live in https://github.com/throgxyz/examples — clone that repo separately.

```bash
git clone https://github.com/throgxyz/examples
cd examples

# Read-only (no key needed) — Nile testnet
cargo run -p examples-queries --example query

# Send TRX
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx

# TRC20 query + transfer
TRON_PRIVATE_KEY=<hex> cargo run -p examples-trc20 --example trc20

# Stake 2.0 (freeze + delegate + claim rewards)
TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example stake

# TRC10 issue new token
TRON_PRIVATE_KEY=<hex> cargo run -p examples-trc10 --example trc10_issue
```

---

## Reference Codebases (local)

Do NOT search the web for these — read them directly from disk.

| Repo | Path | Use for |
|------|------|---------|
| alloy | `/Users/denniszhou/throgxyz/alloy` | Provider/filler/builder patterns, CI setup |
| gotron-sdk | `/Users/denniszhou/throgxyz/gotron-sdk` | TRON gRPC API coverage, proto definitions |
| tronic | `/Users/denniszhou/throgxyz/tronic` | Another TRON Rust SDK, API design reference |

---

## Pre-Release Checklist

Before bumping the version and publishing a release, always update the following files:

- **`CHANGELOG.md`** — move `[Unreleased]` entries into a new versioned section (`[X.Y.Z] - YYYY-MM-DD`).
- **`README.md`** — reflect any new features, examples, or API changes (feature table, quick-start snippets, examples count and list).
- Any other user-facing `.md` files that reference version numbers or feature lists.

---

## Git Commit Style

Do **not** add `Co-Authored-By: Claude` or any AI attribution to commit messages. Commits should appear as authored solely by the human developer.

---

## Notable Divergences from DESIGN.md

The design doc says "HTTP JSON API first; gRPC is `feature = 'grpc'` for later" — the implementation went gRPC-first instead. This is intentional and correct: TRON's primary RPC surface is gRPC.

The design doc lists `rust-version = "1.75"` but the actual MSRV is `1.85` (stable RPITIT requires 1.85).
