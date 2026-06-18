# Getting Started

## Install

For applications, depend on the umbrella crate:

```toml
[dependencies]
tronz = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

The default feature set includes the gRPC transport with TLS, contract support,
and the local signer. Enable optional signer features when needed:

```toml
tronz = { version = "0.1", features = ["signer-mnemonic", "signer-keystore"] }
```

## Connect To Nile

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_NILE, TronProvider};

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc(TRONGRID_NILE)
    .await?;

let block = provider.get_now_block().await?;
println!("Nile head: #{}", block.number);
# Ok(()) }
```

Use `TRONGRID_MAINNET` for mainnet. TronGrid API keys are optional, but they
help avoid rate limits:

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET};

# async fn run() -> tronz::providers::Result<()> {
let api_key = std::env::var("TRON_API_KEY").ok();
let provider = ProviderBuilder::new()
    .maybe_api_key(api_key)
    .on_grpc(TRONGRID_MAINNET)
    .await?;
# Ok(()) }
```

## Run An Example

Read-only examples do not require a private key:

```bash
cargo run -p examples-queries --example query
cargo run -p examples-queries --example list_witnesses
```

Examples that broadcast transactions require a funded Nile private key:

```bash
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx
```
