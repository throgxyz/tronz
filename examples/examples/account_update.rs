//! Set a human-readable account name on-chain.
//!
//! Account names are stored on-chain as UTF-8 bytes. A name can only be set
//! once per account (subsequent calls are rejected by the node).
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key of the account to name
//!   TRON_NAME        — the desired account name (e.g. "alice")
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_NAME=alice cargo run -p examples --example account_update
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let name = std::env::var("TRON_NAME").expect("TRON_NAME env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Current name ──────────────────────────────────────────────────────────

    let account = provider.get_account(me).await?;
    println!("=== Account {} ===", me);
    println!("  current name : {:?}", account.name);

    if !account.name.is_empty() {
        println!("  account already has a name — update will likely be rejected by the node");
    }

    // ── Send update ───────────────────────────────────────────────────────────

    println!("\n=== Setting name to {:?} ===", name);
    let pending = provider.update_account_name().name(&name).send().await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── Verify ───────────────────────────────────────────────────────────────

    let updated = provider.get_account(me).await?;
    println!("\n=== After ===");
    println!("  name : {:?}", updated.name);

    Ok(())
}
