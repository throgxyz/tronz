//! Claim accumulated staking and voting rewards.
//!
//! TRON distributes block production rewards to SRs and voting rewards to
//! anyone who votes for an SR. Rewards accumulate on-chain and must be
//! explicitly claimed. This example shows how to check the pending reward and
//! claim it.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example claim_rewards
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let signer = LocalSigner::from_hex(&key_hex)?;
    let me = signer.address();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Check pending reward ──────────────────────────────────────────────────

    let reward = provider.get_reward(me).await?;
    let balance_before = provider.get_account(me).await?.balance;

    println!("=== Account {} ===", me);
    println!("  balance        : {} TRX", balance_before.as_trx());
    println!(
        "  pending reward : {} TRX ({} sun)",
        reward.as_trx(),
        reward.as_sun()
    );

    if reward.as_sun() == 0 {
        println!("\n  no reward to claim");
        println!("  tip: vote for SR candidates and wait for a voting cycle (~6h)");
        return Ok(());
    }

    // ── Claim ─────────────────────────────────────────────────────────────────

    println!("\n=== Claiming reward ===");
    let pending = provider.claim_rewards().send().await?;
    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);
    println!("  net fee: {} sun", info.net_fee.as_sun());

    // ── Verify ───────────────────────────────────────────────────────────────

    let balance_after = provider.get_account(me).await?.balance;
    println!("\n=== After ===");
    println!("  balance : {} TRX", balance_after.as_trx());
    println!(
        "  gained  : {} TRX",
        (balance_after - balance_before).as_trx()
    );

    let remaining = provider.get_reward(me).await?;
    println!("  pending reward remaining : {} sun", remaining.as_sun());

    Ok(())
}
