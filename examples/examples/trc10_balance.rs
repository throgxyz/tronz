//! Read a TRC10 token balance from an AccountInfo.
//!
//! TRC10 balances are stored in `AccountInfo::trc10_balances` as a
//! `HashMap<String, i64>` — the key is the token ID and the value is the
//! raw (unscaled) amount.
//!
//! No private key required (read-only).
//!
//! Optional env:
//!   TRON_ADDRESS  — address to query (defaults to a well-known account)
//!   TRON_TOKEN_ID — TRC10 token ID to look up (default: "1000001" = BTT)
//!   TRON_API_KEY  — TronGrid API key
//!
//! ```bash
//! cargo run -p examples --example trc10_balance
//! ```

use tronz::{ProviderBuilder, TRONGRID_NILE, TronProvider, providers::ext::Trc10Api as _};

// A well-known address likely to hold BTT.
const DEFAULT_ADDR: &str = "TWd4WrZ9wn84f5x1hZhL4DHvk738ns5jwb";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr_str = std::env::var("TRON_ADDRESS").unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
    let token_id = std::env::var("TRON_TOKEN_ID").unwrap_or_else(|_| "1000001".to_owned());
    let api_key = std::env::var("TRON_API_KEY").ok();

    let address: tronz::Address = addr_str.parse()?;

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Get token metadata ────────────────────────────────────────────────────

    let token_info = provider.get_asset_info(&token_id).await?;
    println!("=== Token #{} ({}) ===", token_info.id, token_info.abbr);
    println!("  decimals : {}", token_info.decimals);

    // ── Method 1: trc10_balance helper ────────────────────────────────────────

    let raw_balance = provider.trc10_balance(address, &token_id).await?;
    println!("\n=== Balance of {} ===", address);
    println!("  raw balance  : {raw_balance}");

    if token_info.decimals > 0 {
        let divisor = 10i64.pow(token_info.decimals as u32);
        println!(
            "  formatted    : {:.prec$} {}",
            raw_balance as f64 / divisor as f64,
            token_info.abbr,
            prec = token_info.decimals as usize
        );
    }

    // ── Method 2: read from AccountInfo directly ──────────────────────────────
    //
    // `AccountInfo::trc10_balances` is a HashMap<String, i64>.
    // Iterate it to show all TRC10 holdings.

    let account = provider.get_account(address).await?;
    println!("\n=== All TRC10 balances ===");
    if account.trc10_balances.is_empty() {
        println!("  (none)");
    } else {
        let mut balances: Vec<_> = account.trc10_balances.iter().collect();
        balances.sort_by_key(|(id, _)| id.parse::<u64>().unwrap_or(0));
        for (id, amount) in &balances {
            println!("  token #{id:<10} : {amount}");
        }
    }

    Ok(())
}
