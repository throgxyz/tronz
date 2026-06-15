//! Unfreeze (unstake) TRX — enters the 3-day waiting queue.
//!
//! After calling `unfreeze`, the TRX enters a pending state for approximately
//! 3 days (varies by network conditions). Use `withdraw_unfreeze` to claim it
//! after the lock expires.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_FREEZE_SUN  — amount to unfreeze in sun (default: 10 TRX = 10_000_000)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example unfreeze
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx,
    primitives::ResourceCode,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let freeze_sun: i64 = std::env::var("TRON_FREEZE_SUN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000_000);

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let amount = Trx::from_sun(freeze_sun)?;

    // ── Pre-flight check ──────────────────────────────────────────────────────

    let account = provider.get_account(me).await?;
    let staked_energy = account
        .frozen_v2
        .iter()
        .filter(|f| f.resource == ResourceCode::Energy)
        .map(|f| f.amount)
        .fold(Trx::ZERO, |acc, a| acc + a);

    println!("=== Account {} ===", me);
    println!("  staked for energy : {} TRX", staked_energy.as_trx());
    println!(
        "  in-progress unfreeze slots : {}",
        account.unfrozen_v2.len()
    );
    for u in &account.unfrozen_v2 {
        println!(
            "    {:?}  {} TRX  expires {} ms",
            u.resource,
            u.amount.as_trx(),
            u.expire_time_ms
        );
    }

    if staked_energy < amount {
        anyhow::bail!(
            "not enough staked: have {} TRX, requested {} TRX",
            staked_energy.as_trx(),
            amount.as_trx()
        );
    }

    // Check available unfreeze slots (max 32).
    let slots = provider.get_available_unfreeze_count(me).await?;
    println!("\n  available unfreeze slots : {slots}/32");
    if slots == 0 {
        anyhow::bail!("no unfreeze slots available — wait for existing unfreezes to complete");
    }

    // ── Unfreeze ──────────────────────────────────────────────────────────────

    println!("\n=== Unfreeze {} energy ===", amount);
    let pending = provider
        .unfreeze_balance()
        .amount(amount)
        .resource(ResourceCode::Energy)
        .send()
        .await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── After ─────────────────────────────────────────────────────────────────

    let after = provider.get_account(me).await?;
    let new_staked = after
        .frozen_v2
        .iter()
        .filter(|f| f.resource == ResourceCode::Energy)
        .map(|f| f.amount)
        .fold(Trx::ZERO, |acc, a| acc + a);

    println!("\n=== After ===");
    println!("  staked for energy       : {} TRX", new_staked.as_trx());
    println!("  in-progress unfreezes   : {}", after.unfrozen_v2.len());
    for u in &after.unfrozen_v2 {
        println!(
            "    {:?}  {} TRX  expires {} ms",
            u.resource,
            u.amount.as_trx(),
            u.expire_time_ms
        );
    }
    println!("\n  Run `withdraw_unfreeze` after the lock period expires to claim the TRX.");

    Ok(())
}
