# tronz

tronz connects applications to the TRON network.

An idiomatic, async-first Rust SDK for TRON — inspired by [alloy](https://github.com/alloy-rs/alloy).

## Installation

Add the `tronz` crate:

```sh
cargo add tronz
```

Or in your `Cargo.toml`:

```toml
tronz = "0.4"
```

The default features include the TLS-enabled gRPC provider, contract bindings,
and local signing. The `full` feature adds mnemonic, keystore, and TIP-712
signing on top; `signer-aws` stays opt-in because it needs an AWS account. A
full list can be found in the
[`tronz` crate's `Cargo.toml`](https://github.com/throgxyz/tronz/blob/main/crates/tronz/Cargo.toml).

## Examples

### Querying the latest block

```rust,no_run
use tronz::{ProviderBuilder, TronProvider, TRONGRID_MAINNET};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let provider = ProviderBuilder::new().connect_grpc(TRONGRID_MAINNET).await?;

let block = provider.get_now_block().await?;
println!("Latest block: {} ({}ms)", block.number, block.timestamp);
# Ok(())
# }
```

### Sending TRX

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, TronProvider, TRONGRID_NILE, parse_trx};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX").expect("valid key");
let from = signer.address();

let provider = ProviderBuilder::new()
    .with_signer(signer)
    .connect_grpc(TRONGRID_NILE)
    .await?;

let pending = provider
    .send_trx()
    .to(from)
    .amount(parse_trx("1")?)
    .send()
    .await?;

let receipt = pending.get_receipt().await?;
println!("Status: {:?}", receipt.status);
# Ok(())
# }
```

### Querying solidified (irreversible) state

`SolidityProvider` targets a TRON SolidityNode (`WalletSolidity`), which only
serves state confirmed by 2/3+ of the super representatives. It is read-only by
construction — no signer, no broadcast — and `wait_for_success` blocks until a
transaction has solidified *and* its execution succeeded.

```rust,no_run
use tronz::{SolidityProvider, TRONGRID_MAINNET_SOLIDITY};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let solidity = SolidityProvider::connect(TRONGRID_MAINNET_SOLIDITY).await?;

let head = solidity.get_now_block().await?;
println!("solidified head: {}", head.number);

let tx_id = std::env::var("TRON_TX_ID")?.parse()?;
let receipt = solidity.wait_for_success(tx_id).await?;
println!("solidified in block {}", receipt.block_number);
# Ok(())
# }
```

### Type-safe contract bindings

`tron_sol!` turns a Solidity interface into typed call and event builders. The
generated code resolves everything through this crate, so no Alloy dependency
has to be added alongside it.

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET, primitives::Address};

tronz::tron_sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address owner) external view returns (uint256);
    }
}

async fn example() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ProviderBuilder::new().connect_grpc(TRONGRID_MAINNET).await?;
    let usdt: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse()?;

    let token = IERC20::new(usdt, provider);
    let balance = token.balanceOf(usdt).call().await?;
    println!("balance: {balance}");
    Ok(())
}
# fn main() {}
```

For more examples, see the [throgxyz/examples](https://github.com/throgxyz/examples) repository.

## Crates

| Crate | Description |
|-------|-------------|
| [`tronz`] | Meta-crate re-exporting all sub-crates |
| [`tronz-abi`] | Native TRON ABI metadata and optional Alloy JSON ABI conversion |
| [`tronz-primitives`] | `Address`, `Trx`, `ResourceCode`, signatures |
| [`tronz-signer`] | `TronSigner`, `TronSignerSync`, `TronNetworkWallet`, `TronWallet`, and `LocalSigner` |
| [`tronz-provider`] | FullNode and SolidityNode transports/providers, fillers, and domain types |
| [`tronz-contract`] | TRC20 / TRC721 bindings, deployment, calls, and event filters |
| [`tronz-sol-macro`] | `tron_sol!` procedural macro for provider-bound contract bindings |
| [`tronz-signer-aws`] | AWS KMS signer (`signer-aws` feature) |

[`tronz`]: https://github.com/throgxyz/tronz/tree/main/crates/tronz
[`tronz-abi`]: https://github.com/throgxyz/tronz/tree/main/crates/abi
[`tronz-primitives`]: https://github.com/throgxyz/tronz/tree/main/crates/primitives
[`tronz-signer`]: https://github.com/throgxyz/tronz/tree/main/crates/signer
[`tronz-provider`]: https://github.com/throgxyz/tronz/tree/main/crates/provider
[`tronz-contract`]: https://github.com/throgxyz/tronz/tree/main/crates/contract
[`tronz-sol-macro`]: https://github.com/throgxyz/tronz/tree/main/crates/sol-macro
[`tronz-signer-aws`]: https://github.com/throgxyz/tronz/tree/main/crates/signer-aws

## Supported Rust Versions (MSRV)

The minimum supported Rust version is **1.91.1**.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md).

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
