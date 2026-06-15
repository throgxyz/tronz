//! Issue (create) a new TRC10 native token on the Nile testnet.
//!
//! TRC10 tokens are issued directly by the TRON protocol — no smart contract
//! needed. After the transaction is confirmed, the network assigns a numeric
//! token ID that you can use with `transfer_trc10` / `trc10_balance`.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key of the issuer (needs TRX for fee)
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_TOKEN_NAME  — full token name      (default: "TronzTestToken")
//!   TRON_TOKEN_ABBR  — token symbol         (default: "TZT")
//!   TRON_TOKEN_SUPPLY — total supply (raw)  (default: 1_000_000_000_000 = 1M with 6 decimals)
//!   TRON_TOKEN_URL   — project URL          (default: "https://github.com/throgxyz/tronz")
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example trc10_issue
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner,
    providers::ext::Trc10Api as _,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let token_name =
        std::env::var("TRON_TOKEN_NAME").unwrap_or_else(|_| "TronzTestToken".to_owned());
    let token_abbr = std::env::var("TRON_TOKEN_ABBR").unwrap_or_else(|_| "TZT".to_owned());
    let total_supply: i64 = std::env::var("TRON_TOKEN_SUPPLY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000_000_000); // 1,000,000 TZT with 6 decimals
    let url = std::env::var("TRON_TOKEN_URL")
        .unwrap_or_else(|_| "https://github.com/throgxyz/tronz".to_owned());

    let signer = LocalSigner::from_hex(&key_hex)?;
    let issuer = signer.address();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let balance = provider.get_account(issuer).await?.balance;
    println!("=== Issuer {} ===", issuer);
    println!("  balance : {} TRX", balance.as_trx());

    // ── Issue token ───────────────────────────────────────────────────────────
    //
    // Key parameters:
    //   precision   — decimal places (6 = same as USDT)
    //   exchange_rate(1, 1) — ICO rate: 1 TRX = 1 token
    //   start_offset_ms — ICO opens 5 minutes from now (required to be future)
    //   duration_ms — ICO lasts 30 days

    println!("\n=== Issuing TRC10 token ===");
    println!("  name         : {token_name}");
    println!("  symbol       : {token_abbr}");
    println!(
        "  total supply : {total_supply} (raw, 6 decimals → {} tokens)",
        total_supply / 1_000_000
    );
    println!("  url          : {url}");

    let pending = provider
        .issue_trc10()
        .name(&token_name)
        .abbr(&token_abbr)
        .description("Issued by the tronz SDK example")
        .url(&url)
        .total_supply(total_supply)
        .precision(6)
        .exchange_rate(1, 1)
        .send()
        .await?;

    println!("\n  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;

    println!("\n=== Receipt ===");
    println!("  status      : {:?}", info.status);
    println!("  energy used : {}", info.energy_usage);
    println!("  energy fee  : {} sun", info.energy_fee.as_sun());

    // ── Look up the assigned token ID ─────────────────────────────────────────

    let issued = provider.get_asset_issue_by_account(issuer).await?;
    if let Some(token) = issued.iter().find(|t| t.name == token_name) {
        println!("\n=== Token issued ===");
        println!("  id      : {}", token.id);
        println!("  name    : {}", token.name);
        println!("  symbol  : {}", token.abbr);
        println!("  supply  : {}", token.total_supply);
        println!("\n  Transfer with:");
        println!(
            "  TRON_PRIVATE_KEY=<key> TRON_TOKEN_ID={} TRON_TO=<addr> TRON_AMOUNT=1000000 \\",
            token.id
        );
        println!("    cargo run -p examples --example trc10_transfer");
    } else {
        println!("\n  (token confirmed but ID not yet indexed — query shortly)");
        println!("  Run: TRON_ADDRESS={issuer} cargo run -p examples --example trc10_balance");
    }

    Ok(())
}
