# tronz — SDK Design Document

> **Status: DESIGN PHASE — no implementation code yet.**
> Approved crate split: 4 crates + 1 umbrella (primitives / signer / provider / contract / tronz).

---

## 0. Goals & Non-Goals

**Goals**
- Idiomatic, async-first Rust SDK for the TRON network.
- Modular workspace: users depend only on the sub-crates they need.
- First-class ergonomic API for energy/bandwidth staking and delegation (the differentiator vs tronic).
- Feature parity with `gotron-sdk` for common operations (see §8).
- Architecture inspired by Alloy's `Provider` / filler / `Signer` patterns; adapted for TRON.
- TRC20/TRC721 via direct reuse of `alloy-sol-types` + `alloy-sol-macro`; no bespoke ABI codec.

**Non-Goals (v0)**
- gRPC/tonic transport — HTTP JSON API first; gRPC is `feature = "grpc"` for later.
- Hardware wallet signers (Ledger/Trezor).
- ZK shielded transfers.
- DEX / exchange contracts.
- BIP39/BIP44 mnemonic derivation.
- WebSocket / pubsub streaming.

---

## 1. Crate Split & Dependency Graph

### 1.1 Overview

```
tronz-primitives          (leaf types, zero I/O, zero proto)
      ↑           ↑
tronz-signer   tronz-provider   (domain types + proto codec + transport + provider)
                    ↑
              tronz-contract    (sol! bindings, TRC20/TRC721, call builders)
                    ↑
                  tronz         (umbrella re-export)
```

`tronz-core` does **not** exist as a separate crate. The domain model (tx types, block types, account types) and the protobuf codec live inside `tronz-provider`. This avoids an extra layer that primarily serves a niche offline-signing use-case.

### 1.2 Workspace `Cargo.toml` skeleton

```toml
[workspace]
members  = ["crates/*"]
resolver = "2"

[workspace.package]
version     = "0.1.0"
edition     = "2021"
rust-version = "1.75"
license     = "MIT OR Apache-2.0"
repository  = "https://github.com/throgxyz/tronz"

[workspace.dependencies]
# internal
tronz-primitives = { path = "crates/tronz-primitives", version = "0.1.0", default-features = false }
tronz-signer     = { path = "crates/tronz-signer",     version = "0.1.0", default-features = false }
tronz-provider   = { path = "crates/tronz-provider",   version = "0.1.0", default-features = false }
tronz-contract   = { path = "crates/tronz-contract",   version = "0.1.0", default-features = false }

# alloy (reused directly)
alloy-primitives = { version = "1", default-features = false, features = ["std", "serde"] }
alloy-sol-types  = { version = "1", default-features = false }
alloy-sol-macro  = { version = "1", default-features = false }

# crypto
k256     = { version = "0.13", default-features = false, features = ["ecdsa"] }
sha2     = { version = "0.10", default-features = false }
sha3     = { version = "0.10", default-features = false }
bs58     = { version = "0.5",  features = ["check"] }

# proto
prost    = { version = "0.13" }

# async
tokio    = { version = "1", features = ["rt", "time"] }
futures  = { version = "0.3" }

# http
reqwest  = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }

# serde
serde      = { version = "1", features = ["derive"] }
serde_json = { version = "1" }

# error
thiserror = { version = "2" }

# misc
tracing = { version = "0.1" }
hex     = { version = "0.4" }
```

### 1.3 Crate-by-Crate Breakdown

#### `tronz-primitives`

Leaf types only. No network I/O, no prost, no tokio.

```
crates/tronz-primitives/src/
  lib.rs
  address.rs       ← Address ([u8;21], 0x41 prefix)
  amount.rs        ← Trx (i64 sun newtype)
  resource.rs      ← ResourceCode enum
  signature.rs     ← RecoverableSignature
  error.rs         ← AddressError, etc.
```

Dependencies: `alloy-primitives`, `k256`, `bs58`, `sha3`, `hex`, `serde`, `thiserror`

**Nothing from tronz depends on tronz-primitives circularly.**

---

#### `tronz-signer`

Signing trait + local key implementation.

```
crates/tronz-signer/src/
  lib.rs
  trait.rs         ← TronSigner trait, NoSigner
  local.rs         ← LocalSigner (k256 SigningKey)
  error.rs
```

Dependencies: `tronz-primitives`, `k256`, `sha2`, `thiserror`

---

#### `tronz-provider`

The main workhorse. Contains:
- TRON domain model (tx, block, account, contract types)
- Protobuf codec (prost-generated types are **private**)
- Transport trait + HTTP implementation
- Provider trait + RootProvider
- Fillers (TAPOS, fee limit, signer)
- ProviderBuilder
- PendingTransaction

```
crates/tronz-provider/src/
  lib.rs
  error.rs

  types/           ← domain model (public)
    transaction.rs ← RawTransaction, SignedTransaction, TransactionRequest, TransactionInfo
    block.rs       ← BlockInfo, BlockHeader (TAPOS extraction helpers)
    account.rs     ← AccountInfo, AccountResource, DelegatedResource, DelegatedResourceIndex
    contract.rs    ← ContractType enum + all contract param structs
    receipt.rs     ← TransactionInfo, Log, ResourceReceipt

  proto/           ← prost-generated code (PRIVATE mod, not pub)
    mod.rs
    tron.rs        ← generated from Tron.proto
    api.rs         ← generated from api.proto
    codec.rs       ← domain ↔ proto conversions (From/TryFrom impls)

  transport/
    mod.rs         ← TronTransport trait
    http/
      mod.rs       ← HttpTransport (reqwest)
      endpoints.rs ← URL path constants for TronGrid/full-node HTTP API
      types.rs     ← serde structs matching HTTP JSON response shapes

  provider/
    mod.rs         ← TronProvider trait
    root.rs        ← RootProvider<T: TronTransport>
    builder.rs     ← ProviderBuilder + JoinFill combinator
    pending.rs     ← PendingTransaction

  fillers/
    mod.rs
    tapos.rs       ← TaposFiller
    fee_limit.rs   ← FeeLimitFiller
    signer.rs      ← SignerFiller<S>

  builders/        ← per-operation typed builders
    transfer.rs    ← TransferBuilder (send TRX)
    freeze.rs      ← FreezeBuilder / UnfreezeBuilder
    delegate.rs    ← DelegateBuilder / UndelegateBuilder
    withdraw.rs    ← WithdrawExpireBuilder / CancelAllUnfreezeBuilder
    rewards.rs     ← WithdrawBalanceBuilder
    permission.rs  ← AccountPermissionUpdateBuilder
```

Dependencies: `tronz-primitives`, `tronz-signer`, `prost`, `sha2`, `reqwest`, `serde`, `serde_json`, `tokio`, `tracing`, `thiserror`, `alloy-primitives`

Feature flags:
- `default = ["http"]`
- `http` — enables reqwest + HttpTransport
- `grpc` — enables tonic + GrpcTransport (later)

---

#### `tronz-contract`

ABI bindings and contract call infrastructure.

```
crates/tronz-contract/src/
  lib.rs
  call_builder.rs   ← CallBuilder<P,C> (read/constant calls)
  tx_builder.rs     ← ContractTxBuilder<P,C> (write/trigger calls)
  instance.rs       ← ContractInstance<P> (generic handle)
  trc20/
    mod.rs          ← sol! ITRC20 interface + Trc20Instance<P>
  trc721/
    mod.rs          ← sol! ITRC721 interface + Trc721Instance<P>  (later)
```

Dependencies: `tronz-primitives`, `tronz-provider`, `alloy-sol-types`, `alloy-sol-macro`

---

#### `tronz` (umbrella)

```
crates/tronz/src/
  lib.rs            ← pub use + feature-gated re-exports
  error.rs          ← unified Error enum
```

```toml
# crates/tronz/Cargo.toml
[features]
default  = ["provider-http", "contract", "signer-local"]
full     = ["default", "provider-grpc"]

provider-http  = ["tronz-provider/http"]
provider-grpc  = ["tronz-provider/grpc"]
contract       = ["dep:tronz-contract"]
signer-local   = []  # LocalSigner always compiled in tronz-signer
```

---

## 2. Domain Types (`tronz-provider/src/types/`)

These are the **public** Rust types users work with. Prost-generated types never appear in any public signature.

### 2.1 Transaction types

```rust
// types/transaction.rs

/// Builder-stage transaction: all fields optional, filled progressively by fillers.
pub struct TransactionRequest {
    // set by operation builder (send_trx, freeze_balance, etc.)
    pub contract:       Option<ContractType>,
    pub fee_limit:      Option<Trx>,
    pub memo:           Option<Vec<u8>>,
    pub permission_id:  Option<i32>,

    // set by TaposFiller
    pub ref_block_bytes: Option<[u8; 2]>,
    pub ref_block_hash:  Option<[u8; 8]>,
    pub expiration:      Option<i64>,   // unix ms
    pub timestamp:       Option<i64>,   // unix ms
}

/// Fully-populated, ready-to-sign transaction.
pub struct RawTransaction {
    pub ref_block_bytes: [u8; 2],
    pub ref_block_hash:  [u8; 8],
    pub expiration:      i64,
    pub timestamp:       i64,
    pub contract:        ContractType,
    pub fee_limit:       Trx,
    pub memo:            Vec<u8>,
    pub permission_id:   i32,
}

impl RawTransaction {
    /// Protobuf-encode to bytes (prost under the hood, not exposed).
    pub fn encode(&self) -> Vec<u8> { ... }

    /// sha256(self.encode()) — the value that gets signed.
    pub fn tx_id(&self) -> TxId { ... }
}

/// Signed transaction ready to broadcast.
pub struct SignedTransaction {
    pub raw:        RawTransaction,
    /// One signature per signer (multisig can have >1).
    pub signatures: Vec<RecoverableSignature>,
}

/// Receipt returned after a transaction is confirmed on-chain.
pub struct TransactionInfo {
    pub tx_id:            TxId,
    pub block_number:     i64,
    pub block_timestamp:  i64,
    pub status:           TxStatus,
    pub energy_usage:     i64,
    pub energy_fee:       Trx,
    pub net_usage:        i64,
    pub net_fee:          Trx,
    pub contract_result:  ContractResult,
    pub contract_address: Address,       // populated for deploy
    pub logs:             Vec<Log>,
    pub revert_reason:    Option<String>,
}

pub enum TxStatus { Success, Failed }
pub enum ContractResult { Success, Revert, OutOfEnergy, /* ... */ }

pub struct Log {
    pub address: Address,
    pub topics:  Vec<B256>,
    pub data:    Bytes,
}
```

### 2.2 Block types

```rust
// types/block.rs

pub struct BlockInfo {
    pub number:    i64,
    pub hash:      B256,
    pub timestamp: i64,   // unix ms
    /// Raw bytes of the block header (needed for TAPOS hash extraction).
    pub(crate) raw_header_bytes: Vec<u8>,
}

impl BlockInfo {
    /// ref_block_bytes = block_number.to_be_bytes()[6..8]
    pub fn ref_block_bytes(&self) -> [u8; 2] { ... }

    /// ref_block_hash = sha256(raw_header_bytes)[8..16]
    pub fn ref_block_hash(&self) -> [u8; 8] { ... }
}
```

### 2.3 Account types

```rust
// types/account.rs

pub struct AccountInfo {
    pub address:  Address,
    pub balance:  Trx,
    pub name:     String,
    pub is_activated: bool,    // false = address exists only as a key, not on-chain
    pub frozen_v2: Vec<FreezeV2>,
    pub unfrozen_v2: Vec<UnfreezeV2>,
    pub votes:    Vec<Vote>,
    pub permissions: AccountPermissions,
}

pub struct FreezeV2 {
    pub resource: ResourceCode,
    pub amount:   Trx,
}

pub struct UnfreezeV2 {
    pub resource:       ResourceCode,
    pub amount:         Trx,
    pub expire_time_ms: i64,
}

pub struct AccountResource {
    // bandwidth
    pub free_bandwidth_used:  i64,
    pub free_bandwidth_limit: i64,
    pub bandwidth_used:       i64,
    pub bandwidth_limit:      i64,
    // energy
    pub energy_used:          i64,
    pub energy_limit:         i64,
    // delegated (v2)
    pub delegated_bandwidth_for_others: Trx,
    pub delegated_energy_for_others:    Trx,
    pub received_bandwidth:             Trx,
    pub received_energy:                Trx,
    // voting power
    pub tron_power_used:  Trx,
    pub tron_power_limit: Trx,
}

pub struct DelegatedResource {
    pub from:                      Address,
    pub to:                        Address,
    pub bandwidth_amount:          Trx,
    pub energy_amount:             Trx,
    pub bandwidth_expire_time_ms:  i64,
    pub energy_expire_time_ms:     i64,
}

pub struct DelegatedResourceIndex {
    pub account:       Address,
    pub from_accounts: Vec<Address>,  // who delegated TO this address
    pub to_accounts:   Vec<Address>,  // who this address delegated TO
}
```

### 2.4 `ContractType` enum

One variant per TRON contract type. The discriminant values match the protobuf enum exactly.

```rust
// types/contract.rs

/// All TRON native contract types. Mirrors proto ContractType discriminants.
pub enum ContractType {
    // v0 — implemented
    Transfer(TransferContract),
    TriggerSmartContract(TriggerSmartContract),
    FreezeBalanceV2(FreezeBalanceV2Contract),
    UnfreezeBalanceV2(UnfreezeBalanceV2Contract),
    DelegateResource(DelegateResourceContract),
    UnDelegateResource(UnDelegateResourceContract),
    WithdrawExpireUnfreeze(WithdrawExpireUnfreezeContract),
    CancelAllUnfreezeV2(CancelAllUnfreezeV2Contract),
    WithdrawBalance(WithdrawBalanceContract),
    AccountPermissionUpdate(AccountPermissionUpdateContract),
    CreateSmartContract(CreateSmartContract),

    // later
    VoteWitness(VoteWitnessContract),
    WitnessCreate(WitnessCreateContract),
    AccountCreate(AccountCreateContract),
    AccountUpdate(AccountUpdateContract),
    AssetIssue(AssetIssueContract),
    // ... all remaining proto variants
}

// --- per-contract param structs ---

pub struct TransferContract {
    pub owner_address: Address,
    pub to_address:    Address,
    pub amount:        Trx,
}

pub struct TriggerSmartContract {
    pub owner_address:    Address,
    pub contract_address: Address,
    pub call_value:       Trx,
    pub data:             Bytes,   // ABI-encoded selector + args
    pub call_token_value: Trx,
    pub token_id:         i64,
}

pub struct FreezeBalanceV2Contract {
    pub owner_address: Address,
    pub frozen_balance: Trx,
    pub resource:      ResourceCode,
}

pub struct UnfreezeBalanceV2Contract {
    pub owner_address:   Address,
    pub unfreeze_balance: Trx,
    pub resource:        ResourceCode,
}

pub struct DelegateResourceContract {
    pub owner_address:    Address,
    pub resource:         ResourceCode,
    pub balance:          Trx,
    pub receiver_address: Address,
    /// None = no lock; Some(secs) = lock period in seconds.
    pub lock_period:      Option<i64>,
}

pub struct UnDelegateResourceContract {
    pub owner_address:    Address,
    pub resource:         ResourceCode,
    pub balance:          Trx,
    pub receiver_address: Address,
}

pub struct WithdrawExpireUnfreezeContract {
    pub owner_address: Address,
}

pub struct CancelAllUnfreezeV2Contract {
    pub owner_address: Address,
}

pub struct WithdrawBalanceContract {
    pub owner_address: Address,
}

pub struct AccountPermissionUpdateContract {
    pub owner_address: Address,
    pub owner:         Option<Permission>,
    pub witness:       Option<Permission>,
    pub actives:       Vec<Permission>,
}

pub struct CreateSmartContract {
    pub owner_address: Address,
    pub bytecode:      Bytes,
    pub abi:           Vec<u8>,     // JSON-encoded ABI
    pub call_value:    Trx,
    pub consume_user_resource_percent: i64,
    pub origin_energy_limit: i64,
    pub name:          String,
}
```

---

## 3. Protobuf Codec (`tronz-provider/src/proto/`)

The proto module is **entirely private** (`mod proto` without `pub`). It contains:

1. **prost-generated code** — compiled from `.proto` files via `build.rs`.
2. **Codec conversions** — `From`/`TryFrom` impls between proto types and domain types.

```
proto/
  mod.rs          ← pub(crate) re-export of generated modules
  codec.rs        ← all domain ↔ proto From/TryFrom impls
  *.rs            ← prost-generated (named by package: tron_core, tron_api, ...)
```

`build.rs` drives compilation:

```rust
// crates/tronz-provider/build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "proto";
    prost_build::Config::new()
        .out_dir("src/proto")
        .compile_protos(
            &[
                "proto/tron/core/Tron.proto",
                "proto/tron/core/tron/transaction.proto",
                "proto/tron/core/contract/balance_contract.proto",
                // ... all contract protos
                "proto/tron/api/api.proto",
            ],
            &[proto_root],
        )?;
    Ok(())
}
```

### Key codec invariants

- Proto → domain: `TryFrom` (can fail on malformed data; returns `ProtoConvError`)
- Domain → proto: `From` (always succeeds; domain types are stricter)
- `RawTransaction::encode()` calls `prost::Message::encode()` on the proto `transaction::Raw`
- `RawTransaction::tx_id()` = `sha256(self.encode())`

---

## 4. Transport Layer (`tronz-provider/src/transport/`)

### 4.1 `TronTransport` trait

TRON is **not** JSON-RPC; we do not reuse alloy's `tower::Service<RequestPacket>`.

```rust
// transport/mod.rs

pub trait TronTransport: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    // --- Block ---
    fn get_now_block(&self)
        -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    fn get_block_by_number(&self, num: i64)
        -> impl Future<Output = Result<BlockInfo, Self::Error>> + Send;

    // --- Account ---
    fn get_account(&self, address: Address)
        -> impl Future<Output = Result<AccountInfo, Self::Error>> + Send;

    fn get_account_resource(&self, address: Address)
        -> impl Future<Output = Result<AccountResource, Self::Error>> + Send;

    // --- Transaction ---
    fn broadcast_transaction(&self, tx: &SignedTransaction)
        -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn get_transaction_by_id(&self, tx_id: TxId)
        -> impl Future<Output = Result<SignedTransaction, Self::Error>> + Send;

    fn get_transaction_info(&self, tx_id: TxId)
        -> impl Future<Output = Result<TransactionInfo, Self::Error>> + Send;

    // --- Smart contracts ---
    fn trigger_smart_contract(&self, params: TriggerSmartContract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn trigger_constant_contract(&self, params: TriggerSmartContract)
        -> impl Future<Output = Result<ConstantCallResult, Self::Error>> + Send;

    fn estimate_energy(&self, params: TriggerSmartContract)
        -> impl Future<Output = Result<i64, Self::Error>> + Send;

    // --- Staking ---
    fn freeze_balance_v2(&self, params: FreezeBalanceV2Contract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn unfreeze_balance_v2(&self, params: UnfreezeBalanceV2Contract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn delegate_resource(&self, params: DelegateResourceContract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn undelegate_resource(&self, params: UnDelegateResourceContract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn withdraw_expire_unfreeze(&self, params: WithdrawExpireUnfreezeContract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn cancel_all_unfreeze_v2(&self, params: CancelAllUnfreezeV2Contract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    fn withdraw_balance(&self, params: WithdrawBalanceContract)
        -> impl Future<Output = Result<RawTransaction, Self::Error>> + Send;

    // --- Resource queries ---
    fn get_delegated_resource(&self, from: Address, to: Address)
        -> impl Future<Output = Result<Vec<DelegatedResource>, Self::Error>> + Send;

    fn get_delegated_resource_index(&self, address: Address)
        -> impl Future<Output = Result<DelegatedResourceIndex, Self::Error>> + Send;

    fn get_can_delegate_max(&self, address: Address, resource: ResourceCode)
        -> impl Future<Output = Result<Trx, Self::Error>> + Send;

    fn get_reward(&self, address: Address)
        -> impl Future<Output = Result<Trx, Self::Error>> + Send;

    // --- Network ---
    fn get_chain_parameters(&self)
        -> impl Future<Output = Result<HashMap<String, i64>, Self::Error>> + Send;

    fn get_contract(&self, address: Address)
        -> impl Future<Output = Result<SmartContractInfo, Self::Error>> + Send;
}
```

**Note on `trigger_smart_contract` return type:** The HTTP API builds the unsigned `RawTransaction` server-side (it fills in TAPOS). We receive the unsigned raw tx back, then sign it locally. This is different from TRX transfers where we build the raw tx entirely client-side.

### 4.2 `HttpTransport`

```rust
// transport/http/mod.rs

#[derive(Clone)]
pub struct HttpTransport {
    client:   reqwest::Client,
    base_url: Url,
    api_key:  Option<String>,
}

impl HttpTransport {
    pub fn new(base_url: impl Into<String>) -> Result<Self, TransportError>;
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self;
}
```

HTTP endpoint mapping (TronGrid-compatible, same as raw full node `/wallet/*`):

| Transport method | HTTP endpoint | Method |
|---|---|---|
| `get_now_block` | `/wallet/getnowblock` | POST |
| `get_block_by_number` | `/wallet/getblockbynum` | POST |
| `get_account` | `/wallet/getaccount` | POST |
| `get_account_resource` | `/wallet/getaccountresource` | POST |
| `broadcast_transaction` | `/wallet/broadcasttransaction` | POST |
| `get_transaction_by_id` | `/wallet/gettransactionbyid` | POST |
| `get_transaction_info` | `/wallet/gettransactioninfobyid` | POST |
| `trigger_smart_contract` | `/wallet/triggersmartcontract` | POST |
| `trigger_constant_contract` | `/wallet/triggerconstantcontract` | POST |
| `estimate_energy` | `/wallet/estimateenergy` | POST |
| `freeze_balance_v2` | `/wallet/freezebalancev2` | POST |
| `unfreeze_balance_v2` | `/wallet/unfreezebalancev2` | POST |
| `delegate_resource` | `/wallet/delegateresource` | POST |
| `undelegate_resource` | `/wallet/undelegateresource` | POST |
| `withdraw_expire_unfreeze` | `/wallet/withdrawexpireunfreeze` | POST |
| `cancel_all_unfreeze_v2` | `/wallet/cancelallunfreezev2` | POST |
| `withdraw_balance` | `/wallet/withdrawbalance` | POST |
| `get_delegated_resource` | `/wallet/getdelegatedresourcev2` | POST |
| `get_delegated_resource_index` | `/wallet/getdelegatedresourceaccountindexv2` | POST |
| `get_can_delegate_max` | `/wallet/getcandelegatedmaxsize` | POST |
| `get_reward` | `/wallet/getreward` | POST |
| `get_chain_parameters` | `/wallet/getchainparameters` | POST |
| `get_contract` | `/wallet/getcontract` | POST |

**API key:** TronGrid requires `TRON-PRO-API-KEY` header. When `api_key` is set, `HttpTransport` injects it into every request.

**Response shapes:** All endpoints return JSON. The `transport/http/types.rs` module defines private serde structs mirroring the HTTP JSON response format, which are then converted to domain types.

```rust
// transport/http/types.rs  (private)
#[derive(Deserialize)]
struct RawBlockJson {
    #[serde(rename = "blockID")]
    block_id: String,
    block_header: BlockHeaderJson,
    // ...
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockHeaderJson {
    raw_data: RawBlockHeaderJson,
    witness_signature: String,
}
// ... etc.
```

**`raw_data_hex` handling:** For transaction responses, the HTTP API returns a `raw_data_hex` field containing the protobuf-encoded `transaction::Raw` as a hex string. We decode it with prost to reconstruct the `RawTransaction`:

```rust
// inside HttpTransport::get_transaction_by_id
let raw_bytes = hex::decode(&json_response.raw_data_hex)?;
let proto_raw = proto::transaction::Raw::decode(raw_bytes.as_ref())?;
let raw_tx = RawTransaction::try_from(proto_raw)?;
```

---

## 5. Provider Layer (`tronz-provider/src/provider/`)

### 5.1 `TronProvider` trait

```rust
// provider/mod.rs

pub trait TronProvider: Clone + Send + Sync + 'static {
    type Transport: TronTransport;

    fn transport(&self) -> &Self::Transport;

    /// Address of the attached signer, if any.
    fn signer_address(&self) -> Option<Address>;

    // ---------- Reads ----------

    fn get_now_block(&self)
        -> impl Future<Output = Result<BlockInfo>> + Send;

    fn get_account(&self, address: Address)
        -> impl Future<Output = Result<AccountInfo>> + Send;

    fn get_account_resource(&self, address: Address)
        -> impl Future<Output = Result<AccountResource>> + Send;

    fn get_transaction(&self, tx_id: TxId)
        -> impl Future<Output = Result<SignedTransaction>> + Send;

    fn get_transaction_info(&self, tx_id: TxId)
        -> impl Future<Output = Result<TransactionInfo>> + Send;

    fn get_delegated_resource(&self, from: Address, to: Address)
        -> impl Future<Output = Result<Vec<DelegatedResource>>> + Send;

    fn get_delegated_resource_index(&self, address: Address)
        -> impl Future<Output = Result<DelegatedResourceIndex>> + Send;

    fn get_can_delegate_max(&self, address: Address, resource: ResourceCode)
        -> impl Future<Output = Result<Trx>> + Send;

    fn get_reward(&self, address: Address)
        -> impl Future<Output = Result<Trx>> + Send;

    fn chain_parameters(&self)
        -> impl Future<Output = Result<HashMap<String, i64>>> + Send;

    fn energy_price(&self)
        -> impl Future<Output = Result<Trx>> + Send;   // derived from chain_parameters

    fn bandwidth_price(&self)
        -> impl Future<Output = Result<Trx>> + Send;   // derived from chain_parameters

    // ---------- Transaction builders (lazy — no I/O until .send()) ----------

    fn send_trx(&self)           -> TransferBuilder<'_, Self>;
    fn freeze_balance(&self)     -> FreezeBuilder<'_, Self>;
    fn unfreeze_balance(&self)   -> UnfreezeBuilder<'_, Self>;
    fn delegate_resource(&self)  -> DelegateBuilder<'_, Self>;
    fn undelegate_resource(&self)-> UndelegateBuilder<'_, Self>;
    fn withdraw_expire_unfreeze(&self) -> WithdrawExpireBuilder<'_, Self>;
    fn cancel_all_unfreeze(&self)-> CancelAllUnfreezeBuilder<'_, Self>;
    fn claim_rewards(&self)      -> WithdrawBalanceBuilder<'_, Self>;

    // ---------- Low-level ----------

    /// Sign and broadcast a pre-built TransactionRequest.
    fn send_transaction(&self, req: TransactionRequest)
        -> impl Future<Output = Result<PendingTransaction<'_, Self>>> + Send;

    /// Broadcast an already-signed transaction.
    fn broadcast(&self, tx: SignedTransaction)
        -> impl Future<Output = Result<PendingTransaction<'_, Self>>> + Send;
}
```

### 5.2 `RootProvider<T>`

```rust
// provider/root.rs

#[derive(Clone)]
pub struct RootProvider<T: TronTransport> {
    inner: Arc<RootProviderInner<T>>,
}

struct RootProviderInner<T> {
    transport:       T,
    signer_address:  Option<Address>,
}

impl<T: TronTransport> RootProvider<T> {
    pub fn new(transport: T) -> Self;
    pub fn new_with_signer(transport: T, signer_address: Address) -> Self;
}
```

`RootProvider<T>` implements `TronProvider`. All builder methods on the trait return builder structs that hold a `&RootProvider`.

### 5.3 `ProviderBuilder` & Fillers

Inspired directly by alloy's `ProviderBuilder` pattern.

```rust
// provider/builder.rs

pub struct ProviderBuilder<F> {
    filler: F,
}

impl ProviderBuilder<Identity> {
    pub fn new() -> Self;
}

impl<F: TxFiller> ProviderBuilder<F> {
    /// Add the TAPOS filler (required for broadcasting).
    pub fn with_tapos(self) -> ProviderBuilder<JoinFill<F, TaposFiller>>;

    /// Add a default fee_limit for TriggerSmartContract calls.
    pub fn with_fee_limit(self, limit: Trx)
        -> ProviderBuilder<JoinFill<F, FeeLimitFiller>>;

    /// Attach a signer. After this, .send() works.
    pub fn with_signer<S: TronSigner>(self, signer: S)
        -> ProviderBuilder<JoinFill<F, SignerFiller<S>>>;

    /// Build with an HTTP transport.
    pub fn on_http(self, url: impl Into<String>)
        -> Result<FilledProvider<HttpTransport, F>>;

    /// Convenience: on_http + api_key in one call.
    pub fn on_http_with_key(
        self,
        url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<FilledProvider<HttpTransport, F>>;
}
```

**`JoinFill<L, R>`** — zero-cost combinator that sequences two fillers:

```rust
pub struct JoinFill<L, R> { left: L, right: R }

impl<L: TxFiller, R: TxFiller> TxFiller for JoinFill<L, R> {
    fn fill(...) -> ... {
        // fill left, then right
    }
}
```

**`TxFiller` trait:**

```rust
pub trait TxFiller: Clone + Send + Sync {
    fn status(&self, tx: &TransactionRequest) -> FillerStatus;

    /// Synchronous fill for fields already available in-memory.
    fn fill_sync(&self, tx: &mut TransactionRequest);

    /// Async fill for fields that require a network call.
    fn fill(
        &self,
        tx: TransactionRequest,
        provider: &impl TronProvider,
    ) -> impl Future<Output = Result<TransactionRequest>> + Send;
}

pub enum FillerStatus {
    Ready,     // all my fields already present
    NeedsWork, // I have fields to fill (async or sync)
    Finished,  // I am a no-op (Identity)
}
```

**`FilledProvider<T, F>`** — the concrete type returned by `ProviderBuilder`:

```rust
/// A provider that automatically applies filler F before every send.
#[derive(Clone)]
pub struct FilledProvider<T: TronTransport, F: TxFiller> {
    inner:  RootProvider<T>,
    filler: F,
}

impl<T: TronTransport, F: TxFiller> TronProvider for FilledProvider<T, F> {
    // send_transaction: applies filler.fill(req, self), then signs + broadcasts
    // all read methods: delegate directly to inner.transport()
}
```

### 5.4 Fillers in Detail

#### `TaposFiller`

```rust
pub struct TaposFiller {
    expiry: Duration,   // default: 60s
}

impl TaposFiller {
    pub fn new() -> Self;
    pub fn with_expiry(expiry: Duration) -> Self;
}

impl TxFiller for TaposFiller {
    fn status(&self, tx: &TransactionRequest) -> FillerStatus {
        if tx.ref_block_bytes.is_none() { FillerStatus::NeedsWork }
        else { FillerStatus::Ready }
    }

    async fn fill(&self, mut tx: TransactionRequest, p: &impl TronProvider)
        -> Result<TransactionRequest>
    {
        let block = p.get_now_block().await?;
        tx.ref_block_bytes = Some(block.ref_block_bytes());
        tx.ref_block_hash  = Some(block.ref_block_hash());
        let now_ms = unix_now_ms();
        tx.timestamp  = Some(now_ms);
        tx.expiration = Some(now_ms + self.expiry.as_millis() as i64);
        Ok(tx)
    }
}
```

#### `FeeLimitFiller`

```rust
pub struct FeeLimitFiller {
    default: Trx,
}

impl TxFiller for FeeLimitFiller {
    fn fill_sync(&self, tx: &mut TransactionRequest) {
        // only set if the contract type needs a fee_limit AND none is already set
        if tx.fee_limit.is_none() && tx.contract_needs_fee_limit() {
            tx.fee_limit = Some(self.default);
        }
    }
}
```

`TriggerSmartContract` and `CreateSmartContract` need `fee_limit`. All native contracts (transfer, freeze, etc.) ignore it.

#### `SignerFiller<S>`

```rust
pub struct SignerFiller<S: TronSigner> {
    signer: S,
}

impl<S: TronSigner> TxFiller for SignerFiller<S> {
    // signing is not part of filling TransactionRequest;
    // instead, SignerFiller stores the signer and it is invoked
    // directly in FilledProvider::send_transaction after fill() completes.
    // The filler's fill() is a no-op; it just marks the provider as having a signer.
}
```

**Signing flow inside `FilledProvider::send_transaction`:**

```
1. filler.fill(req, self)        → RawTransaction (TAPOS + fee_limit filled)
2. raw_tx.encode()               → Vec<u8>
3. raw_tx.tx_id()                → TxId (sha256 of encoded bytes)
4. signer.sign_tx_hash(tx_id)    → RecoverableSignature
5. SignedTransaction { raw_tx, signatures: vec![sig] }
6. transport.broadcast_transaction(&signed_tx)
7. PendingTransaction { tx_id }
```

---

## 6. Transaction Build Flow (end-to-end)

### 6.1 Client-side built transactions (most native contracts)

For TRX transfer, freeze, delegate, etc., the client builds the `RawTransaction` entirely locally:

```
provider.send_trx()
  .to(addr)
  .amount(trx!(10 TRX))
  .send()
  .await?
    │
    ├─ builds TransactionRequest {
    │    contract: Some(ContractType::Transfer { owner, to, amount }),
    │    // all other fields None
    │  }
    │
    ├─ TaposFiller.fill()        → adds ref_block_*, expiration, timestamp
    ├─ FeeLimitFiller.fill_sync()→ no-op (Transfer doesn't need fee_limit)
    │
    ├─ TransactionRequest → RawTransaction::from(req)?
    │
    ├─ tx_id = sha256(raw.encode())
    ├─ sig   = signer.sign_tx_hash(tx_id).await?
    │
    └─ transport.broadcast_transaction(&SignedTransaction { raw, sig })
         → PendingTransaction
```

### 6.2 Server-side built transactions (TriggerSmartContract)

For smart contract calls, the node builds the `RawTransaction` (it knows the contract state):

```
provider.send_contract(contract_addr, ITRC20::transferCall { to, amount })
  .fee_limit(trx!(20 TRX))
  .send()
  .await?
    │
    ├─ transport.trigger_smart_contract(TriggerSmartContract {
    │    owner, contract_addr, data: call.abi_encode(), fee_limit, ...
    │  })  → RawTransaction  (server filled TAPOS fields)
    │
    ├─ tx_id = sha256(raw.encode())
    ├─ sig   = signer.sign_tx_hash(tx_id).await?
    │
    └─ transport.broadcast_transaction(&SignedTransaction { raw, sig })
         → PendingTransaction
```

Note: for TriggerSmartContract the TaposFiller is **skipped** (the server filled TAPOS). We skip it by checking if `ref_block_bytes` is already set in the returned `RawTransaction`.

### 6.3 TAPOS byte extraction

```
Block number (i64, big-endian 8 bytes):
  ref_block_bytes = number.to_be_bytes()[6..8]   ← last 2 bytes of block number

Block raw header bytes:
  ref_block_hash  = sha256(raw_header_bytes)[8..16]  ← bytes 8..16 of hash
```

### 6.4 `PendingTransaction`

```rust
pub struct PendingTransaction<'a, P: TronProvider> {
    provider: &'a P,
    tx_id:    TxId,
}

impl<P: TronProvider> PendingTransaction<'_, P> {
    pub fn tx_id(&self) -> TxId { self.tx_id }

    /// Poll get_transaction_info until status is Success or Failed.
    /// Default: poll every 3s, up to 20 attempts (60s total).
    pub async fn await_confirmed(self) -> Result<TransactionInfo> {
        self.await_confirmed_with(Duration::from_secs(3), 20).await
    }

    pub async fn await_confirmed_with(
        self,
        interval: Duration,
        max_attempts: u32,
    ) -> Result<TransactionInfo> {
        for _ in 0..max_attempts {
            tokio::time::sleep(interval).await;
            match self.provider.get_transaction_info(self.tx_id).await {
                Ok(info) => return Ok(info),
                Err(Error::NotFound) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::ConfirmationTimeout)
    }
}
```

---

## 7. Contract Layer (`tronz-contract`)

### 7.1 `sol!` reuse

```rust
// tronz-contract/src/trc20/mod.rs
use alloy_sol_macro::sol;

sol! {
    #[derive(Debug, PartialEq)]
    interface ITRC20 {
        function name()                                               external view returns (string);
        function symbol()                                             external view returns (string);
        function decimals()                                           external view returns (uint8);
        function totalSupply()                                        external view returns (uint256);
        function balanceOf(address account)                           external view returns (uint256);
        function transfer(address to, uint256 amount)                 external returns (bool);
        function approve(address spender, uint256 amount)             external returns (bool);
        function allowance(address owner, address spender)            external view returns (uint256);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);
    }
}
```

This generates (via `alloy-sol-macro`):
- `ITRC20::transferCall { to: Address, amount: U256 }` with `.abi_encode() -> Vec<u8>`
- `ITRC20::balanceOfReturn { _0: U256 }` with `abi_decode(&bytes) -> Result<Self>`
- etc.

**Address bridging:** `alloy_sol_types::Address` (20 bytes) ↔ `tronz_primitives::Address` (21 bytes):

```rust
impl From<tronz_primitives::Address> for alloy_primitives::Address {
    fn from(a: tronz_primitives::Address) -> Self {
        alloy_primitives::Address::from_slice(a.as_evm_bytes())
    }
}
```

When calling `ITRC20::balanceOfCall { account }`, the `account` must be the 20-byte EVM form. `Address::as_evm_bytes()` strips the 0x41 prefix.

### 7.2 `CallBuilder` (read-only)

```rust
pub struct CallBuilder<'a, P, C: SolCall> {
    provider: &'a P,
    contract: Address,
    call:     C,
    from:     Option<Address>,
}

impl<P: TronProvider, C: SolCall> CallBuilder<'_, P, C> {
    pub fn from(mut self, addr: Address) -> Self;

    pub async fn call(self) -> Result<C::Return> {
        let data = Bytes::from(self.call.abi_encode());
        let params = TriggerSmartContract {
            owner_address:    self.from.unwrap_or_default(),
            contract_address: self.contract,
            data,
            ..Default::default()
        };
        let result = self.provider.transport()
            .trigger_constant_contract(params)
            .await?;
        C::abi_decode_returns(&result.output, true)
            .map_err(Error::AbiDecode)
    }
}
```

### 7.3 `ContractTxBuilder` (write)

```rust
pub struct ContractTxBuilder<'a, P, C: SolCall> {
    provider:      &'a P,
    contract:      Address,
    call:          C,
    call_value:    Trx,
    fee_limit:     Option<Trx>,
    memo:          Option<String>,
    permission_id: Option<i32>,
}

impl<P: TronProvider, C: SolCall> ContractTxBuilder<'_, P, C> {
    pub fn call_value(mut self, v: Trx) -> Self;
    pub fn fee_limit(mut self, v: Trx) -> Self;
    pub fn memo(mut self, m: impl Into<String>) -> Self;

    pub async fn send(self) -> Result<PendingTransaction<'_, P>> {
        let data = Bytes::from(self.call.abi_encode());
        // uses the server-side build path (trigger_smart_contract)
        let raw_tx = self.provider.transport()
            .trigger_smart_contract(TriggerSmartContract {
                owner_address:    self.provider.signer_address()
                                      .ok_or(Error::NoSigner)?,
                contract_address: self.contract,
                data,
                call_value:       self.call_value,
                fee_limit:        self.fee_limit.unwrap_or(Trx::from_trx(15.0)),
                ..Default::default()
            })
            .await?;

        // sign + broadcast
        self.provider.broadcast(sign(raw_tx, &self.provider)?).await
    }
}
```

### 7.4 `Trc20Instance`

```rust
pub struct Trc20Instance<P> {
    address:  Address,
    provider: P,
}

impl<P: TronProvider> Trc20Instance<P> {
    pub fn new(address: Address, provider: P) -> Self;
    pub fn address(&self) -> Address;

    // --- reads ---
    pub async fn name(&self) -> Result<String>;
    pub async fn symbol(&self) -> Result<String>;
    pub async fn decimals(&self) -> Result<u8>;
    pub async fn total_supply(&self) -> Result<U256>;
    pub async fn balance_of(&self, account: Address) -> Result<U256>;
    pub async fn allowance(&self, owner: Address, spender: Address) -> Result<U256>;

    // --- writes (return builder for .fee_limit() / .memo() chaining) ---
    pub fn transfer(&self, to: Address, amount: U256)
        -> ContractTxBuilder<'_, P, ITRC20::transferCall>;
    pub fn approve(&self, spender: Address, amount: U256)
        -> ContractTxBuilder<'_, P, ITRC20::approveCall>;
    pub fn transfer_from(&self, from: Address, to: Address, amount: U256)
        -> ContractTxBuilder<'_, P, ITRC20::transferFromCall>;
}
```

---

## 8. Resource / Staking API

This is the primary differentiator of tronz over existing Rust TRON libraries.

### 8.1 Typed builders

Each builder exposes only the fields relevant to that operation — no raw protobuf exposure.

```rust
/// Stake TRX to obtain energy or bandwidth (Stake 2.0 / FreezeBalanceV2).
pub struct FreezeBuilder<'a, P> {
    provider:  &'a P,
    amount:    Option<Trx>,
    resource:  ResourceCode,       // default: Energy
    owner:     Option<Address>,    // default: provider.signer_address()
}

impl<P: TronProvider> FreezeBuilder<'_, P> {
    pub fn amount(mut self, v: Trx) -> Self;
    pub fn resource(mut self, r: ResourceCode) -> Self;
    pub fn owner(mut self, a: Address) -> Self;
    pub async fn send(self) -> Result<PendingTransaction<'_, P>>;
}

/// Unstake TRX (14-day unbonding period applies).
pub struct UnfreezeBuilder<'a, P> { ... }
impl<P: TronProvider> UnfreezeBuilder<'_, P> {
    pub fn amount(mut self, v: Trx) -> Self;
    pub fn resource(mut self, r: ResourceCode) -> Self;
    pub async fn send(self) -> Result<PendingTransaction<'_, P>>;
}

/// Delegate staked energy or bandwidth to another account.
pub struct DelegateBuilder<'a, P> { ... }
impl<P: TronProvider> DelegateBuilder<'_, P> {
    pub fn resource(mut self, r: ResourceCode) -> Self;
    pub fn amount(mut self, v: Trx) -> Self;
    pub fn to(mut self, a: Address) -> Self;
    /// Lock delegation (cannot be recalled for this many seconds).
    /// Max = 864_000 seconds (10 days) per TRON protocol.
    pub fn lock_period(mut self, secs: i64) -> Self;
    pub async fn send(self) -> Result<PendingTransaction<'_, P>>;
}

/// Reclaim delegated resources.
pub struct UndelegateBuilder<'a, P> { ... }
impl<P: TronProvider> UndelegateBuilder<'_, P> {
    pub fn resource(mut self, r: ResourceCode) -> Self;
    pub fn amount(mut self, v: Trx) -> Self;
    pub fn from(mut self, a: Address) -> Self;   // whose delegation to reclaim
    pub async fn send(self) -> Result<PendingTransaction<'_, P>>;
}

/// Claim TRX from expired unfreeze windows.
pub struct WithdrawExpireBuilder<'a, P> { ... }
impl<P: TronProvider> WithdrawExpireBuilder<'_, P> {
    pub async fn send(self) -> Result<PendingTransaction<'_, P>>;
}

/// Cancel all in-progress unfreeze operations.
pub struct CancelAllUnfreezeBuilder<'a, P> { ... }

/// Claim accumulated block rewards / vote rewards.
pub struct WithdrawBalanceBuilder<'a, P> { ... }
```

### 8.2 Resource query helpers

```rust
// Available directly on TronProvider

// Who delegated to this address & how much?
fn get_delegated_resource(&self, from: Address, to: Address)
    -> impl Future<Output = Result<Vec<DelegatedResource>>> + Send;

// Full index: from_accounts (delegated TO me) + to_accounts (I delegated TO them)
fn get_delegated_resource_index(&self, address: Address)
    -> impl Future<Output = Result<DelegatedResourceIndex>> + Send;

// Max amount this address can still delegate for a resource
fn get_can_delegate_max(&self, address: Address, resource: ResourceCode)
    -> impl Future<Output = Result<Trx>> + Send;

// Pending (unclaimed) block/vote reward
fn get_reward(&self, address: Address)
    -> impl Future<Output = Result<Trx>> + Send;

// Full bandwidth + energy usage/limits
fn get_account_resource(&self, address: Address)
    -> impl Future<Output = Result<AccountResource>> + Send;
```

---

## 9. Error Handling

Each crate owns a `thiserror`-based `Error` enum. No `eyre` in any public API.

```rust
// tronz-primitives
#[derive(thiserror::Error, Debug)]
pub enum AddressError {
    #[error("invalid prefix byte: expected 0x41, got 0x{0:02x}")]
    BadPrefix(u8),
    #[error("bad length: expected 21 bytes, got {0}")]
    BadLength(usize),
    #[error("base58 decode failed: {0}")]
    Base58(#[from] bs58::decode::Error),
    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),
}

// tronz-signer
#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error("signing failed: {0}")]
    Ecdsa(#[from] k256::ecdsa::Error),
    #[error("signer has no address")]
    NoAddress,
}

// tronz-provider (transport layer)
#[derive(thiserror::Error, Debug)]
pub enum TransportError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("node error {code}: {message}")]
    NodeError { code: i32, message: String },
    #[error("protobuf decode error: {0}")]
    Proto(#[from] prost::DecodeError),
    #[error("json decode error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
}

// tronz-provider (provider layer)
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Signer(#[from] tronz_signer::SignerError),
    #[error(transparent)]
    Address(#[from] tronz_primitives::AddressError),
    #[error("no signer attached to this provider")]
    NoSigner,
    #[error("transaction reverted: {0}")]
    Revert(String),
    #[error("timed out waiting for confirmation")]
    ConfirmationTimeout,
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// tronz-contract adds:
#[error("ABI decode error: {0}")]
AbiDecode(alloy_sol_types::Error),
```

---

## 10. Async & Runtime

- **Tokio** is the only runtime. `tokio::time::sleep` is used in `PendingTransaction`.
- **Rust ≥ 1.75** (RPITIT: `-> impl Future` in traits without boxing).
- **No `async_trait` macro** in public traits. `TronTransport` and `TronSigner` use RPITIT.
- `TronProvider` methods return `impl Future + Send` → compatible with multi-threaded `tokio::spawn`.
- Internal use of `Arc<RootProviderInner<T>>` makes `RootProvider` cheap to clone and `Send + Sync`.

---

## 11. What's Reused from Alloy

| Component | Decision | Where |
|---|---|---|
| `alloy-primitives` (B256, U256, Bytes, keccak256) | **Direct dep** | `tronz-primitives`, `tronz-provider` |
| `alloy-sol-macro` (`sol!` proc-macro) | **Direct dep** | `tronz-contract` |
| `alloy-sol-types` (SolCall, abi_encode/decode) | **Direct dep** | `tronz-contract` |
| alloy Transport trait | **Not reused** — JSON-RPC specific | — |
| alloy Network trait | **Not reused** — EVM types embedded | — |
| alloy Provider trait | **Not reused** — eth_* methods | — |
| alloy `ProviderBuilder` + `TxFiller` pattern | **Pattern adapted** | `tronz-provider/provider/builder.rs` |
| alloy `JoinFill` combinator | **Pattern adapted** | `tronz-provider/provider/builder.rs` |
| alloy `PendingTransaction` | **Pattern adapted** | `tronz-provider/provider/pending.rs` |
| tronic `.proto` files | **Copy directly** | `crates/tronz-provider/proto/` |
| tronic domain types | **Redesigned** — cleaner API boundary | `tronz-provider/src/types/` |
| tronic gRPC transport | **Prior art for later** | future `grpc` feature |

---

## 12. Feature-Parity Map vs gotron-sdk

| gotron-sdk capability | tronz API | v0 or later |
|---|---|---|
| **Account** | | |
| GetAccount | `provider.get_account(addr)` | **v0** |
| GetAccountResource | `provider.get_account_resource(addr)` | **v0** |
| CreateAccount (activation) | `provider.send_trx()` — auto-activates | **v0** |
| UpdateAccount | `provider.update_account()` builder | later |
| **Blocks** | | |
| GetNowBlock | `provider.get_now_block()` | **v0** |
| GetBlockByNum | `provider.transport().get_block_by_number(n)` | **v0** |
| GetBlockById | | later |
| **Transfers** | | |
| CreateTransaction (TRX) | `provider.send_trx()` | **v0** |
| Sign + Broadcast | `SignerFiller` + `provider.broadcast()` | **v0** |
| SendAndConfirm | `PendingTransaction::await_confirmed()` | **v0** |
| **Resources / Staking** | | |
| GetAccountResource | `provider.get_account_resource()` | **v0** |
| GetDelegatedResourcesV2 | `provider.get_delegated_resource(from, to)` | **v0** |
| GetDelegatedResourceIndexV2 | `provider.get_delegated_resource_index(addr)` | **v0** |
| GetReceivedDelegatedResourcesV2 | `.from_accounts` field on `DelegatedResourceIndex` | **v0** |
| GetCanDelegatedMaxSize | `provider.get_can_delegate_max(addr, resource)` | **v0** |
| FreezeBalanceV2 | `provider.freeze_balance()` builder | **v0** |
| UnfreezeBalanceV2 | `provider.unfreeze_balance()` builder | **v0** |
| DelegateResource | `provider.delegate_resource()` builder | **v0** |
| UnDelegateResource | `provider.undelegate_resource()` builder | **v0** |
| WithdrawExpireUnfreeze | `provider.withdraw_expire_unfreeze()` builder | **v0** |
| CancelAllUnfreezeV2 | `provider.cancel_all_unfreeze()` builder | **v0** |
| GetReward | `provider.get_reward(addr)` | **v0** |
| WithdrawBalance (claim rewards) | `provider.claim_rewards()` builder | **v0** |
| **Smart Contracts** | | |
| TriggerSmartContract | `provider.send_contract(addr, call)` | **v0** |
| TriggerConstantContract | `provider.call(addr, call).call()` | **v0** |
| EstimateEnergy | `provider.transport().estimate_energy(params)` | **v0** |
| GetContract (ABI) | `provider.transport().get_contract(addr)` | **v0** |
| DeployContract | `provider.deploy_contract()` builder | later |
| **TRC20** | | |
| TRC20ContractBalance | `Trc20Instance::balance_of(addr)` | **v0** |
| TRC20GetName/Symbol/Decimals | `Trc20Instance::name/symbol/decimals()` | **v0** |
| TRC20Send | `Trc20Instance::transfer(to, amount).send()` | **v0** |
| TRC20Approve | `Trc20Instance::approve(spender, amount).send()` | **v0** |
| TRC20TransferFrom | `Trc20Instance::transfer_from(..).send()` | **v0** |
| **TRC721** | | |
| Standard methods | `Trc721Instance` | later |
| **Transaction** | | |
| GetTransactionByID | `provider.get_transaction(tx_id)` | **v0** |
| GetTransactionInfoByID | `provider.get_transaction_info(tx_id)` | **v0** |
| MultiSig | `permission_id` on builders + future `MultiSigner` | later |
| **Network** | | |
| GetChainParameters | `provider.chain_parameters()` | **v0** |
| energy_price / bandwidth_price | `provider.energy_price()` / `provider.bandwidth_price()` | **v0** |
| GetNodeInfo | | later |
| **Keys** | | |
| LocalSigner from bytes/hex | `LocalSigner::from_bytes` / `from_hex` | **v0** |
| Encrypted keystore | `tronz-keystore` crate | later |
| BIP39 mnemonic / HD derivation | `tronz-mnemonic` crate | later |
| Ledger hardware wallet | `tronz-signer-ledger` crate | later |
| **Transport** | | |
| HTTP JSON (TronGrid / full-node) | `HttpTransport` (default) | **v0** |
| gRPC | `feature = "grpc"` | later |
| API key header | `HttpTransport::with_api_key()` | **v0** |
| **Witnesses / Voting** | | |
| VoteWitness | `provider.vote_witness()` builder | later |
| ListWitnesses / GetBrokerage | | later |
| **Proposals** | | |
| CreateProposal / VoteProposal | | later |
| **Exchange (DEX)** | | |
| CreateExchange / Inject / Withdraw | | later |
| **TRC10 Assets** | | |
| Asset transfer / issue | | later |

---

## 13. Open Questions

**OQ-1 — `TronSigner` object safety**
RPITIT returns `impl Future`, which is not object-safe. Options:
- A) `DynSigner(Arc<dyn ErasedSigner + Send + Sync>)` wrapper with blanket `TronSigner` impl.
- B) `async_trait` macro — object-safe, one heap alloc per call.
Recommendation: start with A for zero-cost; add B as an optional `dyn-signer` feature if users ask.

**OQ-2 — HTTP API target**
TronGrid and raw full-node use the same `/wallet/*` endpoint shapes. The only difference is the `TRON-PRO-API-KEY` header. `HttpTransport::with_api_key()` handles this uniformly — no special-casing needed. ✅ Resolved.

**OQ-3 — `raw_data_hex` decode**
Always decode via prost (not JSON fallback). The `raw_data_hex` field is the canonical source of truth for reconstructing `RawTransaction`. ✅ Resolved.

**OQ-4 — Builder crate: `bon` vs manual**
`bon` reduces boilerplate significantly (tronic proves this). Use `bon` for operation builders (FreezeBuilder, DelegateBuilder, etc.) where there are ≥ 3 optional fields. Hand-write simpler builders. Decision needed before implementation.

**OQ-5 — TAPOS expiry default**
Default 60s; configurable globally via `TaposFiller::with_expiry()` and per-transaction via a `.expiry()` method on each builder. Both levels of configurability, consistent default.

**OQ-6 — gRPC feature flag**
`feature = "grpc"` in `tronz-provider` (not a separate crate). Keeps the workspace smaller. `tonic` only compiled when the feature is on.

**OQ-7 — Rust edition**
Edition 2021, `rust-version = "1.75"`. RPITIT is stable since 1.75. Edition 2024 not needed; 2021 has wider CI toolchain coverage.

**OQ-8 — `Trx` inner type: `i64` vs `u64`**
Keep `i64` to match the protobuf `sint64`. Negative values are invalid protocol-wise but should not cause panics during deserialization. `Trx::from_sun(sun: i64)` returns an error for negative values in user-facing constructors.

---

## Appendix A: v0 Build Order

```
1. crates/tronz-primitives    Address, Trx, ResourceCode, RecoverableSignature
2. crates/tronz-signer        TronSigner, LocalSigner
3. crates/tronz-provider
   a. types/                  domain model structs
   b. proto/                  build.rs + prost + codec conversions
   c. transport/http/         HttpTransport
   d. provider/root.rs        RootProvider
   e. fillers/                TaposFiller, FeeLimitFiller, SignerFiller
   f. provider/builder.rs     ProviderBuilder, JoinFill, FilledProvider
   g. provider/pending.rs     PendingTransaction
   h. builders/               TransferBuilder, FreezeBuilder, DelegateBuilder, ...
4. crates/tronz-contract      sol! TRC20, Trc20Instance, CallBuilder, ContractTxBuilder
5. crates/tronz               umbrella + examples
```

---

## Appendix B: Ergonomic API Target

```rust
use tronz::{LocalSigner, ProviderBuilder, Trx, Address, ResourceCode};
use tronz::contract::Trc20Instance;

#[tokio::main]
async fn main() -> tronz::Result<()> {
    let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX")?;

    let provider = ProviderBuilder::new()
        .with_tapos()
        .with_fee_limit(Trx::from_trx(20.0))
        .with_signer(signer)
        .on_http_with_key("https://api.trongrid.io", "API_KEY")?;

    // --- TRX transfer ---
    let receipt = provider
        .send_trx()
        .to("TRecipient...".parse::<Address>()?)
        .amount(Trx::from_trx(1.0))
        .memo("hello tronz")
        .send().await?
        .await_confirmed().await?;
    println!("confirmed in block {}", receipt.block_number);

    // --- TRC20 transfer (USDT) ---
    let usdt = Trc20Instance::new(
        "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse()?,
        &provider,
    );
    let my_addr = provider.signer_address().unwrap();
    let balance = usdt.balance_of(my_addr).await?;
    println!("USDT balance: {balance}");
    usdt.transfer("TRecipient...".parse()?, balance / 2)
        .fee_limit(Trx::from_trx(30.0))
        .send().await?
        .await_confirmed().await?;

    // --- Stake 100 TRX for energy ---
    provider.freeze_balance()
        .amount(Trx::from_trx(100.0))
        .resource(ResourceCode::Energy)
        .send().await?;

    // --- Delegate 50 TRX of energy to another account for 1 day ---
    provider.delegate_resource()
        .resource(ResourceCode::Energy)
        .amount(Trx::from_trx(50.0))
        .to("TDelegatee...".parse()?)
        .lock_period(86_400)
        .send().await?;

    // --- Check delegation index ---
    let index = provider.get_delegated_resource_index(my_addr).await?;
    println!("delegating to {} accounts", index.to_accounts.len());
    println!("receiving from {} accounts", index.from_accounts.len());

    // --- Claim block rewards ---
    provider.claim_rewards().send().await?;

    Ok(())
}
```
