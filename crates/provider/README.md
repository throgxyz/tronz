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
