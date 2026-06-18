# TRC10

TRC10 is TRON's native token standard. It does not use smart-contract ABI calls;
assets are identified by numeric token ids and are handled by protocol-level
contracts.

Import the extension trait to access TRC10 methods:

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_NILE};
use tronz::providers::ext::Trc10Api as _;

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc(TRONGRID_NILE)
    .await?;

let info = provider.get_asset_info("1000001").await?;
println!("{} ({})", info.name, info.abbr);
# Ok(()) }
```

Write operations use builders:

```rust,ignore
provider
    .transfer_trc10()
    .to(recipient)
    .token_id("1000001")
    .amount(1)
    .send()
    .await?;
```

Examples:

- `cargo run -p examples-trc10 --example trc10_query`
- `cargo run -p examples-trc10 --example trc10_balance`
- `TRON_PRIVATE_KEY=<hex> cargo run -p examples-trc10 --example trc10_issue`
