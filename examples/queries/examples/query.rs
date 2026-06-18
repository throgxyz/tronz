//! Read-only queries against the TronGrid Nile testnet.
//!
//! No private key required.
//!
//! ```
//! cargo run -p examples-queries --example query
//! ```
//!
//! Optional env:
//!   TRON_API_KEY   — TronGrid API key (avoids rate-limits)
//!   TRON_ADDRESS   — address to query (defaults to a well-known Nile account)

use tronz::{ProviderBuilder, TRONGRID_NILE, TronProvider, primitives::ResourceCode};

// A well-known TRON address that is present on Nile testnet with TRX balance.
const DEFAULT_ADDR: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api_key = std::env::var("TRON_API_KEY").ok();
    let addr_str = std::env::var("TRON_ADDRESS").unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
    let address = addr_str.parse().expect("valid TRON address");

    // ── Connect ──────────────────────────────────────────────────────────────
    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Latest block ─────────────────────────────────────────────────────────
    let block = provider.get_now_block().await?;
    println!("=== Latest block ===");
    println!("  number    : {}", block.number);
    println!("  timestamp : {} ms", block.timestamp);
    println!("  hash      : 0x{}", hex::encode(block.hash));

    // ── Account ───────────────────────────────────────────────────────────────
    let account = provider.get_account(address).await?;
    println!("\n=== Account {} ===", address);
    println!("  balance     : {} TRX", account.balance.as_trx());
    println!("  name        : {:?}", account.name);
    println!("  activated   : {}", account.is_activated);
    println!("  frozen_v2   : {} entries", account.frozen_v2.len());

    for f in &account.frozen_v2 {
        println!("    {:?}  {} TRX staked", f.resource, f.amount.as_trx());
    }

    // ── Resources ─────────────────────────────────────────────────────────────
    let res = provider.get_account_resource(address).await?;
    println!("\n=== Resources ===");
    println!(
        "  bandwidth  : {}/{} used/limit",
        res.bandwidth_used, res.bandwidth_limit
    );
    println!(
        "  energy     : {}/{} used/limit",
        res.energy_used, res.energy_limit
    );
    println!("  tron_power : {} used", res.tron_power_used.as_sun());

    // ── Delegations ───────────────────────────────────────────────────────────
    let idx = provider.get_delegated_resource_index(address).await?;
    println!("\n=== Delegation index ===");
    println!("  delegating to   : {} accounts", idx.to_accounts.len());
    println!("  receiving from  : {} accounts", idx.from_accounts.len());

    // ── Max delegatable ───────────────────────────────────────────────────────
    let max_energy = provider
        .get_can_delegate_max(address, ResourceCode::Energy)
        .await?;
    let max_bw = provider
        .get_can_delegate_max(address, ResourceCode::Bandwidth)
        .await?;
    println!("\n=== Max delegatable ===");
    println!("  energy    : {} TRX", max_energy.as_trx());
    println!("  bandwidth : {} TRX", max_bw.as_trx());

    // ── Pending reward ────────────────────────────────────────────────────────
    let reward = provider.get_reward(address).await?;
    println!("\n=== Pending reward ===");
    println!("  {} TRX", reward.as_trx());

    Ok(())
}
