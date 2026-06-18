//! Transfer tokens on behalf of an approver using `transferFrom`.
//!
//! The `transferFrom` pattern:
//! 1. Token owner calls `approve(spender, amount)` — sets allowance
//! 2. Spender calls `transferFrom(owner, recipient, amount)` — moves tokens
//!
//! This example executes step 2. The private key provided must be the
//! *spender*, not the token owner. Run `trc20_approve` first to set the
//! allowance.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key of the *spender* (not the token owner)
//!   TRON_CONTRACT    — TRC20 contract address
//!   TRON_FROM        — owner address (the one who approved)
//!   TRON_TO          — recipient of the tokens
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_AMOUNT      — raw token amount to transfer (default: 1)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<spender-key> TRON_CONTRACT=<addr> \
//!   TRON_FROM=<owner-addr> TRON_TO=<recipient> \
//!   cargo run -p examples-trc20 --example trc20_transfer_from
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronSigner, U256, contract::Trc20Ext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let from_str = std::env::var("TRON_FROM").expect("TRON_FROM env var required (token owner)");
    let to_str = std::env::var("TRON_TO").expect("TRON_TO env var required (recipient)");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let amount = std::env::var("TRON_AMOUNT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(U256::from)
        .unwrap_or(U256::from(1u64));

    let signer = LocalSigner::from_hex(&key_hex)?;
    let spender = signer.address();

    let contract: tronz::Address = contract_str.parse()?;
    let owner: tronz::Address = from_str.parse()?;
    let recipient: tronz::Address = to_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let token = provider.trc20(contract);

    // ── Check allowance ───────────────────────────────────────────────────────

    let allowance = token.allowance(owner, spender).await?;
    println!("=== transferFrom ===");
    println!("  contract  : {contract}");
    println!("  owner     : {owner}");
    println!("  spender   : {spender}");
    println!("  recipient : {recipient}");
    println!("  amount    : {amount}");
    println!("  allowance : {allowance}");

    if allowance < amount {
        anyhow::bail!(
            "insufficient allowance: have {allowance}, need {amount} — run trc20_approve first"
        );
    }

    // ── Owner balance before ──────────────────────────────────────────────────

    let balance_before = token.balance_of(owner).await?;
    println!("\n  owner balance before : {balance_before}");

    // ── transferFrom ──────────────────────────────────────────────────────────

    println!("  broadcasting…");
    let pending = token.transfer_from(owner, recipient, amount).await?;
    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status : {:?}", info.status);
    if let Some(ref reason) = info.revert_reason {
        println!("  revert : {reason}");
    }
    println!("  energy used : {}", info.energy_usage);

    // ── After ─────────────────────────────────────────────────────────────────

    let balance_after = token.balance_of(owner).await?;
    let allowance_after = token.allowance(owner, spender).await?;
    println!("\n=== After ===");
    println!("  owner balance   : {balance_after}");
    println!("  remaining allowance : {allowance_after}");

    Ok(())
}
