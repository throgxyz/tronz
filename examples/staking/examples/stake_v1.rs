//! Stake 1.0 (legacy) operations: freeze and unfreeze on the Nile testnet.
//!
//! Stake 1.0 was the original TRON staking mechanism. Unlike Stake 2.0:
//! - `FreezeBalance` freezes TRX and optionally delegates the resource inline.
//! - `UnfreezeBalance` releases **all** staked TRX for a resource immediately (no unbonding delay;
//!   the TRX is returned in the same transaction).
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_FREEZE_SUN  — amount to freeze in sun (default: 10_000_000 = 10 TRX)
//!   TRON_DELEGATE_TO — address to delegate energy to (inline delegation)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples-staking --example stake_v1
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

    // ── Resources before ──────────────────────────────────────────────────────
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

    // ── Freeze V1 (stake 1.0) for energy ─────────────────────────────────────
    // frozen_duration must be 3 days on mainnet (the builder defaults to 3).
    println!("\n=== Freeze V1 {} for Energy ===", amount);
    let mut freeze = provider
        .freeze_balance_v1()
        .amount(amount)
        .resource(ResourceCode::Energy);

    if let Some(receiver) = delegate_to {
        println!("  delegating to : {receiver}");
        freeze = freeze.receiver(receiver);
    }

    let pending = freeze.send().await?;
    println!("  tx_id : 0x{}", hex::encode(pending.tx_id()));
    let info = pending.get_receipt().await?;
    println!("  status: {:?}", info.status);

    // ── Resources after freeze ────────────────────────────────────────────────
    let res_after = provider.get_account_resource(me).await?;
    println!("\n=== Resources after freeze ===");
    println!(
        "  energy    : {}/{}",
        res_after.energy_used, res_after.energy_limit
    );
    println!(
        "  bandwidth : {}/{}",
        res_after.bandwidth_used, res_after.bandwidth_limit
    );

    // ── Unfreeze V1 (releases everything immediately) ─────────────────────────
    println!("\n=== Unfreeze V1 energy ===");
    let pending = provider
        .unfreeze_balance_v1()
        .resource(ResourceCode::Energy)
        .send()
        .await?;
    println!("  tx_id : 0x{}", hex::encode(pending.tx_id()));
    let info = pending.get_receipt().await?;
    println!("  status: {:?}", info.status);

    Ok(())
}
