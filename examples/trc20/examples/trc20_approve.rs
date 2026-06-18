//! Approve a spender and read back the allowance.
//!
//! The ERC-20 / TRC20 approve pattern: you authorize a spender (usually a
//! contract) to transfer up to `amount` tokens on your behalf. The allowance
//! is stored on-chain and consumed by `transferFrom`.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key of the token owner
//!   TRON_CONTRACT    — TRC20 contract address
//!   TRON_TO          — spender address to approve
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_AMOUNT      — allowance in raw token units (default: 1_000_000 = 1 USDT)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_CONTRACT=<addr> TRON_TO=<spender> \
//!   cargo run -p examples-trc20 --example trc20_approve
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronSigner, U256, contract::Trc20Ext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let spender_str = std::env::var("TRON_TO").expect("TRON_TO env var required (spender address)");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let amount = std::env::var("TRON_AMOUNT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(U256::from)
        .unwrap_or(U256::from(1_000_000u64));

    let signer = LocalSigner::from_hex(&key_hex)?;
    let owner = signer.address();

    let contract: tronz::Address = contract_str.parse()?;
    let spender: tronz::Address = spender_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let token = provider.trc20(contract);

    // ── Current allowance ─────────────────────────────────────────────────────

    let before = token.allowance(owner, spender).await?;
    println!("=== Allowance ===");
    println!("  owner   : {owner}");
    println!("  spender : {spender}");
    println!("  before  : {before}");

    // ── Approve ───────────────────────────────────────────────────────────────

    println!("\n=== Approving {amount} ===");
    let pending = token.approve(spender, amount).await?;
    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);
    if let Some(ref reason) = info.revert_reason {
        println!("  revert : {reason}");
    }

    // ── Read back ─────────────────────────────────────────────────────────────

    let after = token.allowance(owner, spender).await?;
    println!("\n=== After ===");
    println!("  allowance : {after}");
    assert_eq!(after, amount, "allowance should match approved amount");

    Ok(())
}
