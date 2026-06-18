# Transactions

tronz follows an alloy-style builder flow:

1. Start with a provider.
2. Choose a typed operation builder.
3. Set required fields.
4. Call `.send().await?`.
5. Await the `PendingTransaction` receipt.

## TRX Transfer

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, Trx};

# async fn run() -> tronz::providers::Result<()> {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX")?;
let to = "TRecipientAddress".parse()?;

let provider = ProviderBuilder::new()
    .with_recommended_fillers()
    .with_signer(signer)
    .on_grpc(TRONGRID_NILE)
    .await?;

let pending = provider
    .send_trx()
    .to(to)
    .amount(Trx::from_sun(1_000_000)?)
    .send()
    .await?;

let receipt = pending.get_receipt().await?;
println!("status: {:?}", receipt.status);
# Ok(()) }
```

## Defaults And Overrides

Most builders infer `.from(...)` from the attached signer. Pass `.from(address)`
only when the provider has permission to sign for that address.

Native TRON operations are built locally, then filled and signed. Smart contract
operations ask the node to build the raw transaction first, then sign locally.
This is why `TaposFiller` can skip transactions that already contain TAPOS
fields.

## Receipts

`PendingTransaction::get_receipt()` polls transaction info until the node indexes
the transaction or the polling window expires. `NotFound` is treated as
"not confirmed yet"; other transport errors abort polling.
