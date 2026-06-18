//! Transfer TRX on the Nile testnet and wait for confirmation.
//!
//! Required env:
//!   TRON_PRIVATE_KEY  — hex private key (no 0x prefix)
//!   TRON_TO           — recipient address (defaults to sending to self)
//!
//! Optional env:
//!   TRON_API_KEY      — TronGrid API key
//!   TRON_AMOUNT_SUN   — amount in sun (default: 1_000_000 = 1 TRX)
//!
//! ```
//! TRON_PRIVATE_KEY=<key> cargo run -p examples-transfers --example transfer_trx
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let amount_sun: i64 = std::env::var("TRON_AMOUNT_SUN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000); // 1 TRX default

    let signer = LocalSigner::from_hex(&key_hex)?;
    let from = signer.address();

    let to: tronz::Address = std::env::var("TRON_TO")
        .ok()
        .map(|s| s.parse().expect("valid TRON_TO address"))
        .unwrap_or(from); // send to self by default

    let amount = Trx::from_sun(amount_sun)?;

    // ── Connect ──────────────────────────────────────────────────────────────
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Pre-flight balance check ──────────────────────────────────────────────
    let before = provider.get_account(from).await?.balance;
    println!("From    : {from}");
    println!("To      : {to}");
    println!("Amount  : {amount}");
    println!("Balance : {} TRX (before)", before.as_trx());

    // ── Build and send ────────────────────────────────────────────────────────
    println!("\nBroadcasting…");
    let pending = provider.send_trx().to(to).amount(amount).send().await?;

    let tx_id = pending.tx_id();
    println!("tx_id   : 0x{}", hex::encode(tx_id));

    // ── Wait for confirmation ─────────────────────────────────────────────────
    println!("Waiting for confirmation…");
    let info = pending.get_receipt().await?;

    println!("\n=== Confirmed ===");
    println!("  block       : {}", info.block_number);
    println!("  status      : {:?}", info.status);
    println!("  energy used : {}", info.energy_usage);
    println!("  net used    : {}", info.net_usage);
    println!("  net fee     : {} sun", info.net_fee.as_sun());

    // ── Post-flight balance ───────────────────────────────────────────────────
    let after = provider.get_account(from).await?.balance;
    println!("\nBalance : {} TRX (after)", after.as_trx());

    Ok(())
}
