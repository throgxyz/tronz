//! Delegate energy (or bandwidth) to another account on the Nile testnet.
//!
//! Delegation lets you share your staked resources with another address —
//! useful for paying for a dApp user's transactions without transferring TRX.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key of the delegating account
//!   TRON_DELEGATE_TO — address to receive the delegation
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_FREEZE_SUN  — amount to delegate in sun (default: 10 TRX = 10_000_000)
//!   TRON_LOCK_DAYS   — lock delegation for N days (0–10; default: 0 = no lock)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_DELEGATE_TO=<addr> cargo run -p examples --example delegate
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx,
    primitives::ResourceCode,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let to_str = std::env::var("TRON_DELEGATE_TO").expect("TRON_DELEGATE_TO env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let freeze_sun: i64 = std::env::var("TRON_FREEZE_SUN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000_000);
    let lock_days: i64 = std::env::var("TRON_LOCK_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();
    let receiver: tronz::Address = to_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let amount = Trx::from_sun(freeze_sun)?;

    // ── Check how much is delegatable ─────────────────────────────────────────

    let max = provider
        .get_can_delegate_max(me, ResourceCode::Energy)
        .await?;
    println!("=== Delegation ===");
    println!("  from            : {me}");
    println!("  to              : {receiver}");
    println!("  requested       : {amount}");
    println!("  max delegatable : {} TRX (energy)", max.as_trx());

    if amount > max {
        anyhow::bail!(
            "requested {} TRX but max delegatable is {} TRX — stake more energy first",
            amount.as_trx(),
            max.as_trx()
        );
    }

    // ── Current delegations to receiver ──────────────────────────────────────

    let current = provider.get_delegated_resource(me, receiver).await?;
    if !current.is_empty() {
        println!("\n=== Existing delegations to {receiver} ===");
        for d in &current {
            if d.energy_amount.as_sun() > 0 {
                println!("  energy    : {} TRX", d.energy_amount.as_trx());
                if d.energy_expire_time_ms > 0 {
                    println!("  locked until: {} ms", d.energy_expire_time_ms);
                }
            }
            if d.bandwidth_amount.as_sun() > 0 {
                println!("  bandwidth : {} TRX", d.bandwidth_amount.as_trx());
            }
        }
    }

    // ── Delegate ──────────────────────────────────────────────────────────────

    println!("\n=== Delegating {} energy to {} ===", amount, receiver);

    let mut builder = provider
        .delegate_resource()
        .resource(ResourceCode::Energy)
        .amount(amount)
        .to(receiver);

    if lock_days > 0 {
        // Lock period is in seconds: 1 day = 86_400 seconds.
        // Max lock is 10 days (864_000 seconds).
        let lock_secs = lock_days.min(10) * 86_400;
        builder = builder.lock_period(lock_secs);
        println!("  lock period : {lock_days} day(s) ({lock_secs}s)");
    }

    let pending = builder.send().await?;
    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── Verify ───────────────────────────────────────────────────────────────

    let after = provider.get_delegated_resource(me, receiver).await?;
    for d in &after {
        if d.energy_amount.as_sun() > 0 {
            println!("\n=== Confirmed ===");
            println!(
                "  energy delegated to {receiver}: {} TRX",
                d.energy_amount.as_trx()
            );
        }
    }

    Ok(())
}
