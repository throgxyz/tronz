//! Activate a new account on the Nile testnet.
//!
//! On TRON, a key pair only becomes a real account once it has received at
//! least 1 TRX. Activation is a one-time transaction paid for by an existing
//! funded account.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — funded account paying for activation
//!   TRON_TO          — new address to activate (must not already exist on-chain)
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_TO=<new-addr> cargo run -p examples --example account_create
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx};

/// Cost of activating a new account (1 TRX).
const ACTIVATION_FEE_SUN: i64 = 1_000_000;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let new_addr_str = std::env::var("TRON_TO").expect("TRON_TO env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let signer = LocalSigner::from_hex(&key_hex)?;
    let payer = signer.address();
    let new_addr: tronz::Address = new_addr_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Check payer balance ───────────────────────────────────────────────────

    let payer_account = provider.get_account(payer).await?;
    println!("=== Payer ===");
    println!("  address : {payer}");
    println!("  balance : {} TRX", payer_account.balance.as_trx());

    if payer_account.balance < Trx::from_sun(ACTIVATION_FEE_SUN)? {
        anyhow::bail!(
            "payer balance too low: need at least {} TRX",
            ACTIVATION_FEE_SUN as f64 / 1_000_000.0
        );
    }

    // ── Check if already activated ────────────────────────────────────────────

    let target = provider.get_account(new_addr).await?;
    println!("\n=== Target account {} ===", new_addr);
    if target.is_activated {
        println!("  already activated — nothing to do");
        return Ok(());
    }
    println!("  not yet activated");

    // ── Activate by sending 1 TRX ─────────────────────────────────────────────
    //
    // Sending TRX to an address that doesn't exist automatically activates it.
    // The first transfer is the activation transaction.

    let amount = Trx::from_sun(ACTIVATION_FEE_SUN)?;
    println!("\n=== Activating with {} ===", amount);
    println!("  broadcasting…");

    let pending = provider
        .send_trx()
        .to(new_addr)
        .amount(amount)
        .send()
        .await?;
    println!("  tx_id   : 0x{}", hex::encode(pending.tx_id()));

    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status  : {:?}", info.status);
    println!("  net fee : {} sun", info.net_fee.as_sun());

    // ── Confirm activation ────────────────────────────────────────────────────

    let after = provider.get_account(new_addr).await?;
    println!("\n=== Result ===");
    println!("  activated : {}", after.is_activated);
    println!("  balance   : {} TRX", after.balance.as_trx());

    Ok(())
}
