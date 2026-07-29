# tronz-provider

The workhorse crate of the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

Owns the gRPC transports, the high-level [`TronProvider`] trait with its typed
operation builders, and the read-only [`SolidityProvider`]. The domain model
itself belongs to [`tronz-rpc-types`], and is re-exported here as
[`types`](crate::types) so a provider is the only dependency needed to use it.

Contract metadata is exposed as [`TronAbi`](crate::types::TronAbi) so protobuf
information is preserved without forcing provider-only users to depend on Alloy
ABI types.

[`tronz-rpc-types`]: tronz_rpc_types

## Usage

```rust,no_run
use tronz_provider::{ProviderBuilder, TronProvider};
use tronz_provider::transport::grpc::TRONGRID_MAINNET;

# async fn run() -> tronz_provider::Result<()> {
let provider = ProviderBuilder::new().connect_grpc(TRONGRID_MAINNET).await?;
let block = provider.get_now_block().await?;
println!("latest block: {}", block.number);
# Ok(()) }
```

### One provider type for the node and for tests

The transport is erased inside `RootProvider`, so a provider's type says nothing
about how it reaches the node — the same named type is driven by a real node or by
`MockTransport`:

```rust,no_run
use tronz_provider::{ProviderBuilder, ReadProvider, TronProvider};
use tronz_provider::transport::grpc::{GrpcTransport, TRONGRID_MAINNET};

# async fn run() -> tronz_provider::Result<()> {
let transport = GrpcTransport::connect(TRONGRID_MAINNET).await?;
let provider: ReadProvider = ProviderBuilder::new().connect_transport(transport);

println!("latest block: {}", provider.get_now_block().await?.number);
# Ok(()) }
```

`WalletProvider` is the same over a signing stack. What still shows in these types
is the filler chain; `.erased()` drops that too, at one pointer hop per call:

```rust,no_run
use tronz_provider::{DynProvider, ProviderBuilder, TronProvider};
# use tronz_signer::TronWallet;

# async fn run(wallet: TronWallet) -> tronz_provider::Result<()> {
let a = ProviderBuilder::new().connect("grpc.trongrid.io:50051").await?.erased();
let b = ProviderBuilder::new().wallet(wallet).connect("grpc.trongrid.io:50051").await?.erased();

// Stacked differently, but one type.
let providers: Vec<DynProvider> = vec![a, b];
for provider in &providers {
    println!("{}", provider.get_now_block().await?.number);
}
# Ok(()) }
```

### Layers

`TronProvider` is implementable downstream. A wrapper says what it wraps in three
lines — `root()`, `inner()`, and `inner_read()` — overrides the handful of calls it
cares about, and the rest keep travelling down the stack on their own:

```rust,no_run
use tronz_provider::{ProviderBuilder, TronProvider, layers::LoggingLayer};

# async fn run() -> tronz_provider::Result<()> {
let provider = ProviderBuilder::new()
    .layer(LoggingLayer)
    .connect("grpc.trongrid.io:50051")
    .await?;

println!("{}", provider.get_now_block().await?.number);
# Ok(()) }
```

Every default method asks `inner()` (or `inner_read()`, for the contract reads)
before it reaches the transport, so a call the outer layer says nothing about still
runs an inner layer's version of it. One gap
remains: a `PendingTransaction` holds the root provider, so polling for a receipt
does not run the layers the transaction was sent through — Alloy's
`PendingTransactionBuilder` holds a `RootProvider` for the same reason.

Layers suit behaviour attached to particular operations, since a layer has to name
the methods it wants. Behaviour that has to see every RPC belongs in middleware, one
level down, where it needs to name nothing:

```rust,no_run
use async_trait::async_trait;
use tronz_provider::transport::grpc::{GrpcCall, GrpcMiddleware, GrpcOutcome, GrpcTransport};

struct Metrics;

#[async_trait]
impl GrpcMiddleware for Metrics {
    async fn after(&self, call: GrpcCall<'_>, outcome: GrpcOutcome<'_>) {
        println!("{} took {:?} over {} attempts", call.method, outcome.elapsed, outcome.attempts);
    }
}

# async fn run() -> Result<(), tronz_provider::TransportErrorKind> {
let transport = GrpcTransport::builder()
    .with_middleware(std::sync::Arc::new(Metrics))
    .connect("grpc.trongrid.io:50051")
    .await?;
# let _ = transport;
# Ok(()) }
```

`before` can hold a call back, which is how you stay inside a node's rate limit
even for the calls you do not make yourself — receipt polling, event watching.
Middleware sees timing and outcomes, not payloads; to answer calls without a node,
use `MockTransport`.

### Waiting for confirmation

`send()` returns a `PendingTransaction`. Its polling schedule and whether a
reverted transaction counts as an error are configured separately from where the
receipt is read, so the two terminals below accept any combination. The timeout is
a wall clock: it covers the RPCs as well as the waits between them.

```rust,no_run
use core::time::Duration;

use tronz_provider::{PendingTransaction, PendingTransactionError};

# async fn run(pending: PendingTransaction) -> Result<(), PendingTransactionError> {
let pending = pending
    .with_poll_interval(Duration::from_secs(1))
    .with_timeout(Duration::from_secs(30))
    .require_success();

// A FullNode has indexed it. Fast, but a reorg can still drop it.
let receipt = pending.get_receipt().await?;
println!("confirmed in block {}", receipt.block_number);
# Ok(()) }
```

Swap in `get_solidified_receipt(&solidity)` to wait for irreversible state
instead — same configuration, and it trails the head by about a minute.

### Solidified state

```rust,no_run
use tronz_provider::{
    SolidityProvider,
    transport::grpc::TRONGRID_MAINNET_SOLIDITY,
};

# async fn run() -> tronz_provider::Result<()> {
let provider = SolidityProvider::connect(TRONGRID_MAINNET_SOLIDITY).await?;
let block = provider.get_now_block().await?;
println!("solidified block: {}", block.number);
# Ok(()) }
```

### Multisig

Every transaction builder has a `.build()` exit next to `.send()`, which stops
at the unsigned transaction so more than one key can sign it. Add
`.permission_id(id)` when the signing keys belong to an active permission rather
than to the account's owner permission.

```rust,no_run
use tronz_provider::{ProviderBuilder, TronProvider};
use tronz_provider::transport::grpc::TRONGRID_MAINNET;
use tronz_provider::types::SignedTransaction;
use tronz_signer::{LocalSigner, TronNetworkWallet, TronWallet};

# async fn run(
#     key_a: &str,
#     key_b: &str,
#     multisig_account: tronz_primitives::Address,
#     to: tronz_primitives::Address,
# ) -> tronz_provider::Result<()> {
let a = LocalSigner::from_hex(key_a).unwrap();
let b = LocalSigner::from_hex(key_b).unwrap();
let mut wallet = TronWallet::new(a.clone());
wallet.register_signer(b.clone());

let provider =
    ProviderBuilder::new().wallet(wallet.clone()).connect_grpc(TRONGRID_MAINNET).await?;

let raw = provider
    .send_trx()
    .from(multisig_account)
    .to(to)
    .amount("100".parse().unwrap())
    .permission_id(2)
    .build()
    .await?;

let keys = [a.address(), b.address()];
let signatures = wallet.sign_hash_with_many(&keys, &raw.tx_id()).await.unwrap();
let signed = SignedTransaction { raw, signatures };

// Confirm the signatures reach the threshold before spending bandwidth on them.
let weight = provider.get_transaction_sign_weight(&signed).await?;
if weight.current_weight >= weight.required_weight {
    provider.broadcast(signed).await?;
}
# Ok(()) }
```

The wallet collects signatures but does not decide which keys are needed — it
cannot see the account's permissions. To choose them, and to check the threshold
without a round-trip, ask the permission itself:
`Permission::is_satisfied_by` and `weight_of_all` do the same arithmetic locally
that `get_transaction_sign_weight` does on the node.

Both FullNode providers and `SolidityProvider` implement
[`ContractReadProvider`], the shared capability used by contract calls, energy
estimation, and event queries. State freshness follows the provider: FullNode
reads latest available state, while SolidityNode reads irreversible state.

`SolidityProvider` also exposes solidified witness queries — `list_witnesses`
and `get_paginated_now_witness_list(offset, limit)` (the latter returns SRs
sorted by real-time vote count) — plus solidified stake/delegation reads:
`get_delegated_resource[_v1]`, `get_delegated_resource_index[_v1]`,
`get_can_delegate_max`, `get_available_unfreeze_count`, and
`get_can_withdraw_unfreeze_amount` — all mirroring the FullNode `TronProvider`
methods.

## Crate layout

| Module | Description |
|--------|-------------|
| [`types`] | Public TRON domain model (accounts, blocks, transactions, contracts) |
| [`transport`] | [`TronTransport`] / [`SolidityTransport`] traits and gRPC implementations |
| [`fillers`] | Composable transaction fillers (energy, TAPOS, fee limit, wallet) |
| [`builders`] | Typed per-operation builders (`TransferBuilder`, `FreezeBuilder`, …) |

## Fixture replay tests

The gRPC decode paths are exercised offline against real protobuf bytes stored
under `src/transport/grpc/fixtures/`. These fixtures are regenerated by a
manual, `#[ignore]`d capture tool (`src/transport/grpc/capture.rs`) that talks
to a live node; the replay tests are no-ops until the fixtures are committed.

```bash
TRONZ_CAPTURE_URL=http://<host>:50051 \
  TRONZ_TX_SUCCESS=<txid_hex> TRONZ_TX_REVERTED=<txid_hex> \
  cargo test -p tronz-provider --all-features \
    -- --ignored transport::grpc::capture --nocapture
git add crates/provider/src/transport/grpc/fixtures/*.bin
```

See the `capture` module docs for the full list of environment variables.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
