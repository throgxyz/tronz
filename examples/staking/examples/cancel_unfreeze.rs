//! Cancel all in-progress unfreeze operations and re-stake immediately.
//!
//! If you started an unfreeze but changed your mind before the lock expires,
//! `cancel_all_unfreeze` returns all pending-unfreeze TRX to your staked
//! balance in a single transaction.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples-staking --example cancel_unfreeze
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

    // ── Check pending unfreezes ────────────────────────────────────────────────

    let account = provider.get_account(me).await?;
    println!("=== Pending unfreezes ===");
    if account.unfrozen_v2.is_empty() {
        println!("  none — nothing to cancel (run `unfreeze` first)");
        return Ok(());
    }

    let total_pending: tronz::Trx = account
        .unfrozen_v2
        .iter()
        .map(|u| u.amount)
        .fold(tronz::Trx::ZERO, |acc, a| acc + a);

    for u in &account.unfrozen_v2 {
        println!(
            "  {:?}  {} TRX  expires {} ms",
            u.resource,
            u.amount.as_trx(),
            u.expire_time_ms
        );
    }
    println!("  total pending : {} TRX", total_pending.as_trx());

    // ── Cancel all ────────────────────────────────────────────────────────────
    //
    // All pending unfreezes are cancelled in one transaction, regardless of
    // resource type.

    println!("\n=== Cancelling all in-progress unfreezes ===");
    let pending = provider.cancel_all_unfreeze().send().await?;
    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── After ─────────────────────────────────────────────────────────────────

    let after = provider.get_account(me).await?;
    let re_staked: tronz::Trx = after
        .frozen_v2
        .iter()
        .map(|f| f.amount)
        .fold(tronz::Trx::ZERO, |acc, a| acc + a);

    println!("\n=== After ===");
    println!("  pending unfreezes : {}", after.unfrozen_v2.len());
    println!("  total re-staked   : {} TRX", re_staked.as_trx());

    Ok(())
}
