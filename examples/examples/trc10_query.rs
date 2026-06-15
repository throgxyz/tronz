//! Query TRC10 asset metadata by asset ID.
//!
//! TRC10 is TRON's native token standard, predating TRC20. Each token is
//! identified by a numeric ID (e.g. `1000001` for BitTorrent/BTT). Unlike
//! TRC20, TRC10 tokens are handled natively by the TRON protocol — no smart
//! contract involved.
//!
//! No private key required (read-only).
//!
//! Optional env:
//!   TRON_TOKEN_ID — TRC10 asset numeric ID (default: "1000001" = BTT)
//!   TRON_API_KEY  — TronGrid API key
//!
//! ```bash
//! cargo run -p examples --example trc10_query
//! cargo run -p examples --example trc10_query  # (uses BTT by default)
//! TRON_TOKEN_ID=1000016 cargo run -p examples --example trc10_query  # WIN token
//! ```

use tronz::{ProviderBuilder, TRONGRID_NILE, providers::ext::Trc10Api as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token_id = std::env::var("TRON_TOKEN_ID").unwrap_or_else(|_| "1000001".to_owned());
    let api_key = std::env::var("TRON_API_KEY").ok();

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Fetch token metadata ──────────────────────────────────────────────────

    let info = provider.get_asset_info(&token_id).await?;

    println!("=== TRC10 Token #{} ===", info.id);
    println!("  name         : {}", info.name);
    println!("  symbol       : {}", info.abbr);
    println!("  decimals     : {}", info.decimals);
    println!("  total supply : {}", info.total_supply);
    println!("  issuer       : {}", info.owner);
    println!("  url          : {}", info.url);

    // ── Display normalized supply ─────────────────────────────────────────────

    if info.decimals > 0 {
        let divisor = 10i64.pow(info.decimals as u32);
        let whole = info.total_supply / divisor;
        let frac = info.total_supply % divisor;
        println!(
            "  supply (fmt) : {whole}.{frac:0>width$} {}",
            info.abbr,
            width = info.decimals as usize
        );
    }

    // ── Browse tokens on-chain ────────────────────────────────────────────────
    //
    // You can page through all issued TRC10 tokens:

    println!("\n=== First 5 TRC10 tokens ===");
    let tokens = provider.get_asset_issue_list(0, 5).await?;
    for t in &tokens {
        println!("  #{:<10}  {:<12}  {}", t.id, t.abbr, t.name);
    }

    Ok(())
}
