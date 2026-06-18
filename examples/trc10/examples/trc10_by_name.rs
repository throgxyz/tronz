//! Look up TRC10 tokens by name (exact-match and list-by-name).
//!
//! TRON allows querying TRC10 tokens by their human-readable name in addition
//! to their numeric ID. Multiple tokens can share the same name, so
//! `get_asset_issue_list_by_name` returns all matches.
//!
//! No private key required (read-only).
//!
//! Optional env:
//!   TRON_TOKEN_NAME — token name to look up (default: "BitTorrent")
//!   TRON_API_KEY    — TronGrid API key
//!
//! ```bash
//! cargo run -p examples-trc10 --example trc10_by_name
//! TRON_TOKEN_NAME=WIN cargo run -p examples-trc10 --example trc10_by_name
//! ```

use tronz::{ProviderBuilder, TRONGRID_NILE, providers::ext::Trc10Api as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token_name = std::env::var("TRON_TOKEN_NAME").unwrap_or_else(|_| "BitTorrent".to_owned());
    let api_key = std::env::var("TRON_API_KEY").ok();

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Exact match (first token with this name) ──────────────────────────────

    println!("=== Exact match for \"{token_name}\" ===");
    match provider.get_asset_issue_by_name(&token_name).await {
        Ok(info) => {
            println!("  id           : #{}", info.id);
            println!("  name         : {}", info.name);
            println!("  symbol       : {}", info.abbr);
            println!("  decimals     : {}", info.decimals);
            println!("  total supply : {}", info.total_supply);
            println!("  issuer       : {}", info.owner);
            println!("  url          : {}", info.url);
        }
        Err(e) => println!("  not found: {e}"),
    }

    // ── All tokens with this name ─────────────────────────────────────────────

    println!("\n=== All tokens named \"{token_name}\" ===");
    match provider.get_asset_issue_list_by_name(&token_name).await {
        Ok(list) if list.is_empty() => println!("  (none found)"),
        Ok(list) => {
            for t in &list {
                println!("  #{:<10}  {:<12}  issuer={}", t.id, t.abbr, t.owner);
            }
        }
        Err(e) => println!("  error: {e}"),
    }

    Ok(())
}
