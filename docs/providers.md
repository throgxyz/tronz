# Providers

`TronProvider` is the primary user-facing trait. It exposes read methods,
transaction builders, and low-level send/broadcast operations over a
`TronTransport`.

## Read-Only Provider

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET, TronProvider};

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc(TRONGRID_MAINNET)
    .await?;

let block = provider.get_now_block().await?;
let witnesses = provider.list_witnesses().await?;
# Ok(()) }
```

## Signed Provider

Write operations need a signer and the recommended filler chain:

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE};

# async fn run() -> tronz::providers::Result<()> {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX")?;

let provider = ProviderBuilder::new()
    .with_recommended_fillers()
    .with_signer(signer)
    .on_grpc(TRONGRID_NILE)
    .await?;
# Ok(()) }
```

`with_recommended_fillers()` adds:

- `TaposFiller` - fills block reference fields and expiration.
- `FeeLimitFiller` - sets a default smart-contract fee limit.

`with_signer()` adds `SignerFiller`, which signs the transaction before
broadcast.

## Endpoints

| Constant | Network | Endpoint |
| --- | --- | --- |
| `TRONGRID_MAINNET` | Mainnet | `https://grpc.trongrid.io:443` |
| `TRONGRID_NILE` | Nile testnet | `http://grpc.nile.trongrid.io:50051` |

Use `ProviderBuilder::connect(uri)` as an alias for `on_grpc(uri)`.
