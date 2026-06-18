# Governance And Witnesses

TRON block producers are super representatives (SRs). tronz exposes both read
APIs and signed builders for witness and governance workflows.

## Witnesses

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET, TronProvider};

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc(TRONGRID_MAINNET)
    .await?;

let mut witnesses = provider.list_witnesses().await?;
witnesses.sort_by_key(|w| std::cmp::Reverse(w.vote_count));
for witness in witnesses.iter().take(10) {
    println!("{} {}", witness.address, witness.vote_count);
}
# Ok(()) }
```

Use `WitnessApi` for SR-specific operations such as brokerage lookup,
becoming a witness, updating a witness URL, and updating brokerage.

## Governance

Import `GovernanceApi` to list proposals and submit approval transactions:

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET};
use tronz::providers::ext::GovernanceApi as _;

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc(TRONGRID_MAINNET)
    .await?;

let proposals = provider.list_proposals().await?;
println!("{} proposals", proposals.len());
# Ok(()) }
```

Example:

```bash
cargo run -p examples-queries --example governance_list
```
