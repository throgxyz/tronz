//! Withdraw TRX from expired unfreeze entries.
//!
//! After the ~3 day waiting period, unfrozen TRX can be withdrawn back to your
//! spendable balance. This example checks what is withdrawable and, if any,
//! claims it.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example withdraw_unfreeze
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
        println!("  none — run `unfreeze` first");
        return Ok(());
    }
    for u in &account.unfrozen_v2 {
        println!(
            "  {:?}  {} TRX  expires {} ms",
            u.resource,
            u.amount.as_trx(),
            u.expire_time_ms
        );
    }

    // ── Check how much is withdrawable now ────────────────────────────────────

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;

    let withdrawable = provider
        .get_can_withdraw_unfreeze_amount(me, now_ms)
        .await?;
    println!("\n=== Withdrawable now ===");
    println!("  {} TRX", withdrawable.as_trx());

    if withdrawable.as_sun() == 0 {
        println!("  nothing withdrawable yet — check back after the lock expires");
        return Ok(());
    }

    // ── Withdraw ──────────────────────────────────────────────────────────────

    let balance_before = provider.get_account(me).await?.balance;
    println!("\n  balance before : {} TRX", balance_before.as_trx());

    println!("  broadcasting withdraw…");
    let pending = provider.withdraw_expire_unfreeze().send().await?;
    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);

    // ── After ─────────────────────────────────────────────────────────────────

    let balance_after = provider.get_account(me).await?.balance;
    println!("\n  balance after  : {} TRX", balance_after.as_trx());
    println!(
        "  gained         : {} TRX",
        (balance_after - balance_before).as_trx()
    );

    Ok(())
}
