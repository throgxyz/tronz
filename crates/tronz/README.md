# tronz

tronz connects applications to the TRON network.

An idiomatic, async-first Rust SDK for TRON — inspired by [alloy](https://github.com/alloy-rs/alloy).

## Installation

Add the `tronz` crate with the `full` feature flag:

```sh
cargo add tronz --features full
```

Or in your `Cargo.toml`:

```toml
tronz = { version = "0.1", features = ["full"] }
```

A full list of available features can be found in the
[`tronz` crate's `Cargo.toml`](https://github.com/throgxyz/tronz/blob/main/crates/tronz/Cargo.toml).

## Examples

### Querying the latest block

```rust,no_run
use tronz::{ProviderBuilder, TronProvider, TRONGRID_MAINNET};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let provider = ProviderBuilder::new().on_grpc(TRONGRID_MAINNET).await?;

let block = provider.get_now_block().await?;
println!("Latest block: {} ({}ms)", block.number, block.timestamp);
# Ok(())
# }
```

### Sending TRX

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, TronProvider, TronSigner, Trx, TRONGRID_NILE};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX").expect("valid key");
let from = signer.address();

let provider = ProviderBuilder::new()
    .with_recommended_fillers()
    .with_signer(signer)
    .on_grpc(TRONGRID_NILE)
    .await?;

let pending = provider
    .send_trx()
    .to(from)
    .amount(Trx::from_sun_unchecked(1_000_000))
    .send()
    .await?;

let receipt = pending.get_receipt().await?;
println!("Status: {:?}", receipt.status);
# Ok(())
# }
```

For more examples, see the workspace [`examples/`](https://github.com/throgxyz/tronz/tree/main/examples) directory.

## Crates

| Crate | Description |
|-------|-------------|
| [`tronz`] | Meta-crate re-exporting all sub-crates |
| [`tronz-primitives`] | `Address`, `Trx`, `ResourceCode`, signatures |
| [`tronz-signer`] | `TronSigner` trait and `LocalSigner` implementation |
| [`tronz-provider`] | Transport, provider, fillers, and domain types |
| [`tronz-contract`] | TRC20 ABI bindings and dynamic contract helpers |

[`tronz`]: https://github.com/throgxyz/tronz/tree/main/crates/tronz
[`tronz-primitives`]: https://github.com/throgxyz/tronz/tree/main/crates/primitives
[`tronz-signer`]: https://github.com/throgxyz/tronz/tree/main/crates/signer
[`tronz-provider`]: https://github.com/throgxyz/tronz/tree/main/crates/provider
[`tronz-contract`]: https://github.com/throgxyz/tronz/tree/main/crates/contract

## Supported Rust Versions (MSRV)

The minimum supported Rust version is **1.90**.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md).

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
