# tronz-provider

The workhorse crate of the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

Owns the public TRON domain model, the gRPC transports, the high-level
[`TronProvider`] trait with its typed operation builders, and the read-only
[`SolidityProvider`].

Contract metadata is exposed as [`tronz_abi::TronAbi`] so protobuf information
is preserved without forcing provider-only users to depend on Alloy ABI types.

## Usage

```rust,no_run
use tronz_provider::{ProviderBuilder, TronProvider};
use tronz_provider::transport::grpc::TRONGRID_MAINNET;

# async fn run() -> tronz_provider::Result<()> {
let provider = ProviderBuilder::new().on_grpc(TRONGRID_MAINNET).await?;
let block = provider.get_now_block().await?;
println!("latest block: {}", block.number);
# Ok(()) }
```

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
| [`fillers`] | Composable transaction fillers (TAPOS, fee limit, signer) |
| [`builders`] | Typed per-operation builders (`TransferBuilder`, `FreezeBuilder`, …) |

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
