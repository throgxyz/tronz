//! Stake 2.0 operations: freeze, delegate, claim rewards on the Nile testnet.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_DELEGATE_TO — address to delegate energy to after staking
//!                      (defaults to self — no delegation step)
//!   TRON_FREEZE_SUN  — amount to freeze in sun (default: 10_000_000 = 10 TRX)
//!
//! ```
//! TRON_PRIVATE_KEY=<key> cargo run -p examples-staking --example stake
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
        .unwrap_or(10_000_000); // 10 TRX

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();

    let delegate_to: Option<tronz::Address> = std::env::var("TRON_DELEGATE_TO")
        .ok()
        .map(|s| s.parse().expect("valid TRON_DELEGATE_TO address"));

    // ── Connect ──────────────────────────────────────────────────────────────
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let amount = Trx::from_sun(freeze_sun)?;

    // ── Current resources ─────────────────────────────────────────────────────
    let res_before = provider.get_account_resource(me).await?;
    println!("=== Resources before ===");
    println!(
        "  energy    : {}/{}",
        res_before.energy_used, res_before.energy_limit
    );
    println!(
        "  bandwidth : {}/{}",
        res_before.bandwidth_used, res_before.bandwidth_limit
    );

    // ── Pending rewards ───────────────────────────────────────────────────────
    let reward = provider.get_reward(me).await?;
    println!("\n=== Pending reward ===");
    println!("  {} TRX", reward.as_trx());

    if reward.as_sun() > 0 {
        println!("  Claiming rewards…");
        let pending = provider.claim_rewards().send().await?;
        println!("  tx_id : 0x{}", hex::encode(pending.tx_id()));
        let info = pending.get_receipt().await?;
        println!("  status: {:?}", info.status);
    }

    // ── Freeze (stake) for energy ─────────────────────────────────────────────
    println!("\n=== Freeze {} for Energy ===", amount);
    let pending = provider
        .freeze_balance()
        .amount(amount)
        .resource(ResourceCode::Energy)
        .send()
        .await?;
    println!("  tx_id : 0x{}", hex::encode(pending.tx_id()));
    let info = pending.get_receipt().await?;
    println!("  status: {:?}", info.status);

    // ── Delegate energy (if requested) ────────────────────────────────────────
    if let Some(receiver) = delegate_to {
        println!("\n=== Delegate {} energy to {} ===", amount, receiver);
        let max = provider
            .get_can_delegate_max(me, ResourceCode::Energy)
            .await?;
        println!("  max delegatable energy: {} TRX", max.as_trx());

        let delegate_amount = amount.min(max);
        if delegate_amount.as_sun() > 0 {
            let pending = provider
                .delegate_resource()
                .resource(ResourceCode::Energy)
                .amount(delegate_amount)
                .to(receiver)
                .send()
                .await?;
            println!("  tx_id : 0x{}", hex::encode(pending.tx_id()));
            let info = pending.get_receipt().await?;
            println!("  status: {:?}", info.status);
        } else {
            println!("  nothing to delegate");
        }
    }

    // ── Resources after ───────────────────────────────────────────────────────
    let res_after = provider.get_account_resource(me).await?;
    println!("\n=== Resources after ===");
    println!(
        "  energy    : {}/{}",
        res_after.energy_used, res_after.energy_limit
    );
    println!(
        "  bandwidth : {}/{}",
        res_after.bandwidth_used, res_after.bandwidth_limit
    );

    Ok(())
}
