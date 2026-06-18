# Local Nodes

Use the `provider-grpc` feature when connecting to a local or private node over
plain HTTP/2 without TLS:

```toml
tronz = { version = "0.1", default-features = false, features = ["provider-grpc", "contract"] }
```

Then connect to the local endpoint:

```rust,no_run
use tronz::{ProviderBuilder, TronProvider};

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc("http://127.0.0.1:50051")
    .await?;

let block = provider.get_now_block().await?;
println!("{}", block.number);
# Ok(()) }
```

The Nile constant also uses plain HTTP/2 because its hostname is not covered by
TronGrid's wildcard TLS certificate:

```rust,no_run
use tronz::TRONGRID_NILE;
```
