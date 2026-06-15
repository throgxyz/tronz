//! Stake TRX for bandwidth on the Nile testnet.
//!
//! Bandwidth is used by all transactions. Each account gets 600 free bandwidth
//! per day. Staking TRX for bandwidth gives additional bandwidth proportional
//! to the staked amount relative to the total staked TRX on-chain.
//!
//! Required env:
//!   TRON_PRIVATE_KEY  — hex private key
//!
//! Optional env:
//!   TRON_API_KEY      — TronGrid API key
//!   TRON_FREEZE_SUN   — amount to freeze in sun (default: 10 TRX = 10_000_000 sun)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example stake_bandwidth
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

    // ── Before ────────────────────────────────────────────────────────────────

    let res_before = provider.get_account_resource(me).await?;
    println!("=== Bandwidth before staking ===");
    println!(
        "  free     : {}/{}",
        res_before.free_bandwidth_used, res_before.free_bandwidth_limit
    );
    println!(
        "  staked   : {}/{}",
        res_before.bandwidth_used, res_before.bandwidth_limit
    );
    println!(
        "  delegated out : {} TRX",
        res_before.delegated_bandwidth_for_others.as_trx()
    );
    println!(
        "  received      : {} TRX",
        res_before.received_bandwidth.as_trx()
    );

    // ── Freeze for bandwidth ──────────────────────────────────────────────────

    println!("\n=== Freeze {} for Bandwidth ===", amount);
    let pending = provider
        .freeze_balance()
        .amount(amount)
        .resource(ResourceCode::Bandwidth)
        .send()
        .await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);
    println!("  net fee: {} sun", info.net_fee.as_sun());

    // ── After ─────────────────────────────────────────────────────────────────

    let res_after = provider.get_account_resource(me).await?;
    println!("\n=== Bandwidth after staking ===");
    println!(
        "  free     : {}/{}",
        res_after.free_bandwidth_used, res_after.free_bandwidth_limit
    );
    println!(
        "  staked   : {}/{}",
        res_after.bandwidth_used, res_after.bandwidth_limit
    );

    let gained = res_after.bandwidth_limit - res_before.bandwidth_limit;
    println!(
        "  gained   : {} bandwidth units from {} TRX staked",
        gained,
        amount.as_trx()
    );

    Ok(())
}
