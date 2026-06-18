//! Query a TRC20 token and optionally transfer tokens on the Nile testnet.
//!
//! The read-only section runs with no private key. Set TRON_PRIVATE_KEY to
//! also execute a transfer.
//!
//! Required for reads:
//!   TRON_CONTRACT   — TRC20 contract address
//!   TRON_ADDRESS    — address to check balance of
//!
//! Additional for writes:
//!   TRON_PRIVATE_KEY — hex private key
//!   TRON_TO          — transfer recipient (defaults to self)
//!   TRON_AMOUNT      — token units to transfer as a raw uint256 integer (default: 1)
//!
//! Optional:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```
//! TRON_CONTRACT=<addr> TRON_ADDRESS=<addr> cargo run -p examples-trc20 --example trc20
//! ```

use tronz::{LocalSigner, ProviderBuilder, TRONGRID_NILE, TronSigner, U256, contract::Trc20Ext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let contract_str = std::env::var("TRON_CONTRACT")
        .expect("TRON_CONTRACT env var required (TRC20 contract address)");
    let contract: tronz::Address = contract_str.parse()?;

    let query_addr_str =
        std::env::var("TRON_ADDRESS").expect("TRON_ADDRESS env var required (address to query)");
    let query_addr: tronz::Address = query_addr_str.parse()?;

    let api_key = std::env::var("TRON_API_KEY").ok();
    let key_hex = std::env::var("TRON_PRIVATE_KEY").ok();

    // ── Read-only section (no signer needed) ─────────────────────────────────
    {
        let ro_provider = ProviderBuilder::new()
            .maybe_api_key(api_key.clone())
            .on_grpc(TRONGRID_NILE)
            .await?;
        let token = ro_provider.trc20(contract);

        println!("=== TRC20 token {} ===", contract);
        println!("  name         : {}", token.name().await?);
        println!("  symbol       : {}", token.symbol().await?);
        println!("  decimals     : {}", token.decimals().await?);
        println!("  total_supply : {}", token.total_supply().await?);

        let balance = token.balance_of(query_addr).await?;
        println!("\n=== Balance of {} ===", query_addr);
        println!("  {} (raw units)", balance);
    }

    // ── Write section (requires TRON_PRIVATE_KEY) ─────────────────────────────
    let Some(key) = key_hex else {
        println!("\nSet TRON_PRIVATE_KEY to execute a transfer.");
        return Ok(());
    };

    let signer = LocalSigner::from_hex(&key)?;
    let from = signer.address();

    let to: tronz::Address = std::env::var("TRON_TO")
        .ok()
        .map(|s| s.parse().expect("valid TRON_TO address"))
        .unwrap_or(from);

    let amount = std::env::var("TRON_AMOUNT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(U256::from)
        .unwrap_or(U256::from(1u64));

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;
    let token = provider.trc20(contract);

    // Balance before
    let before = token.balance_of(from).await?;
    println!("\n=== Transfer {} units from {} → {} ===", amount, from, to);
    println!("  balance before : {}", before);

    // Transfer
    println!("  broadcasting…");
    let pending = token.transfer(to, amount).await?;
    println!("  tx_id          : 0x{}", hex::encode(pending.tx_id()));

    // Wait for confirmation
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("  status         : {:?}", info.status);
    println!("  contract result: {:?}", info.contract_result);
    if let Some(reason) = &info.revert_reason {
        println!("  revert reason  : {}", reason);
    }
    println!("  energy used    : {}", info.energy_usage);
    println!("  energy fee     : {} sun", info.energy_fee.as_sun());

    // Balance after
    let after = token.balance_of(from).await?;
    println!("  balance after  : {}", after);

    Ok(())
}
