//! Reclaim previously delegated energy from another account.
//!
//! Undelegation is immediate for unlocked delegations. Locked delegations
//! must wait for the lock period to expire.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key of the delegating account
//!   TRON_DELEGATE_TO — address that received the delegation
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_FREEZE_SUN  — amount to reclaim in sun (default: all available energy)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_DELEGATE_TO=<addr> cargo run -p examples-staking --example undelegate
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx,
    primitives::ResourceCode,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let from_str = std::env::var("TRON_DELEGATE_TO").expect("TRON_DELEGATE_TO env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();
    let receiver: tronz::Address = from_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Check current delegation ───────────────────────────────────────────────

    let delegations = provider.get_delegated_resource(me, receiver).await?;
    println!("=== Current delegation from {} to {} ===", me, receiver);

    let delegated_energy = delegations
        .iter()
        .map(|d| d.energy_amount)
        .fold(Trx::ZERO, |acc, a| acc + a);

    if delegated_energy.as_sun() == 0 {
        println!("  no energy delegated to {receiver}");
        return Ok(());
    }

    println!("  energy delegated : {} TRX", delegated_energy.as_trx());

    // Check for lock expiry.
    let locked = delegations.iter().any(|d| d.energy_expire_time_ms > 0);
    if locked {
        let expire_ms = delegations
            .iter()
            .filter_map(|d| (d.energy_expire_time_ms > 0).then_some(d.energy_expire_time_ms))
            .min()
            .unwrap_or(0);
        println!("  WARNING: delegation is locked until {expire_ms} ms — undelegate may fail");
    }

    // ── Amount to reclaim ─────────────────────────────────────────────────────

    let amount: Trx = std::env::var("TRON_FREEZE_SUN")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|sun| Trx::from_sun(sun).expect("valid sun amount"))
        .unwrap_or(delegated_energy);

    println!("  reclaiming       : {} TRX", amount.as_trx());

    // ── Undelegate ────────────────────────────────────────────────────────────

    let pending = provider
        .undelegate_resource()
        .resource(ResourceCode::Energy)
        .amount(amount)
        .from(receiver)
        .send()
        .await?;

    println!("\n  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── Verify ───────────────────────────────────────────────────────────────

    let after = provider.get_delegated_resource(me, receiver).await?;
    let remaining = after
        .iter()
        .map(|d| d.energy_amount)
        .fold(Trx::ZERO, |acc, a| acc + a);
    println!("\n=== After ===");
    println!("  energy still delegated : {} TRX", remaining.as_trx());

    Ok(())
}
