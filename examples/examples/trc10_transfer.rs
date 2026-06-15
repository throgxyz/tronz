//! Transfer a TRC10 token to another address on the Nile testnet.
//!
//! TRC10 transfers are native protocol operations — no smart contract
//! execution, very low bandwidth cost. Get test TRC10 tokens from the
//! Nile faucet or by issuing your own asset.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!   TRON_TO          — recipient address
//!   TRON_TOKEN_ID    — numeric token ID to transfer (e.g. "1000001" for BTT)
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_AMOUNT      — raw amount to send (default: 1)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_TO=<addr> TRON_TOKEN_ID=<id> \
//!   cargo run -p examples --example trc10_transfer
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronSigner, providers::ext::Trc10Api as _,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let to_str = std::env::var("TRON_TO").expect("TRON_TO env var required");
    let token_id = std::env::var("TRON_TOKEN_ID").expect("TRON_TOKEN_ID env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let amount: i64 = std::env::var("TRON_AMOUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    let signer = LocalSigner::from_hex(&key_hex)?;
    let from = signer.address();
    let to: tronz::Address = to_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Token info + balance check ────────────────────────────────────────────

    let token_info = provider.get_asset_info(&token_id).await?;
    let balance_before = provider.trc10_balance(from, &token_id).await?;

    println!("=== TRC10 Transfer ===");
    println!("  token   : #{} ({})", token_info.id, token_info.abbr);
    println!("  from    : {from}");
    println!("  to      : {to}");
    println!("  amount  : {amount} (raw)");
    println!("  balance : {balance_before} (raw, before)");

    if balance_before < amount {
        anyhow::bail!("insufficient balance: have {balance_before}, need {amount}");
    }

    // ── Send ──────────────────────────────────────────────────────────────────

    println!("\n  broadcasting…");
    let pending = provider
        .transfer_trc10()
        .to(to)
        .token_id(&token_id)
        .amount(amount)
        .send()
        .await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);
    println!("  net fee: {} sun", info.net_fee.as_sun());

    // ── After ─────────────────────────────────────────────────────────────────

    let balance_after = provider.trc10_balance(from, &token_id).await?;
    println!("\n=== After ===");
    println!("  balance : {balance_after} (raw)");

    Ok(())
}
