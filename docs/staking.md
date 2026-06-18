# Staking And Resources

TRON uses bandwidth for all transactions and energy for smart-contract
execution. Staking TRX can provide either resource.

## Stake 2.0

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, ResourceCode, TRONGRID_NILE, TronProvider, Trx};

# async fn run() -> tronz::providers::Result<()> {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX")?;
let provider = ProviderBuilder::new()
    .with_recommended_fillers()
    .with_signer(signer)
    .on_grpc(TRONGRID_NILE)
    .await?;

provider
    .freeze_balance()
    .amount(Trx::from_trx(10.0)?)
    .resource(ResourceCode::Energy)
    .send()
    .await?;
# Ok(()) }
```

Stake 2.0 supports separate operations for freezing, unfreezing, delegating,
undelegating, cancelling unfreeze windows, and withdrawing expired unfreeze
amounts.

## Legacy Stake 1.0

Stake 1.0 is still exposed for accounts and flows that depend on the legacy
contracts:

- `freeze_balance_v1()`
- `unfreeze_balance_v1()`
- `get_delegated_resource_v1()`
- `get_delegated_resource_index_v1()`

Examples:

- `TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example stake`
- `TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example stake_v1`
- `TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example delegate`
