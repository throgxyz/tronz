//! Constant contract call — read state without spending energy.
//!
//! `trigger_constant_contract` simulates execution and returns output without
//! creating a transaction. It's the TRON equivalent of `eth_call`.
//!
//! This example calls a TRC20 contract three ways:
//!
//! 1. **`Trc20Instance`** — high-level typed API, no ABI file
//! 2. **Raw calldata** — manually ABI-encode selector and decode output
//! 3. **Dynamic `Interface`** — load a JSON ABI and call by function name
//!
//! No private key required (read-only).
//!
//! Required env:
//!   TRON_CONTRACT — TRC20 contract address (e.g. USDT on mainnet)
//!
//! Optional env:
//!   TRON_API_KEY  — TronGrid API key
//!
//! ```bash
//! TRON_CONTRACT=TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t \
//!   cargo run -p examples --example contract_call
//! ```

use alloy_dyn_abi::DynSolValue;
use alloy_json_abi::JsonAbi;
use tronz::{
    ProviderBuilder, TRONGRID_NILE,
    contract::{ContractExt, Interface, SolCall, Trc20Ext, trc20::ITRC20},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let contract: tronz::Address = contract_str.parse()?;

    // ── Approach 1: Trc20Instance (typed, no ABI file) ───────────────────────

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key.clone())
        .on_grpc(TRONGRID_NILE)
        .await?;

    let token = provider.trc20(contract);
    println!("=== Trc20Instance (typed calls) ===");
    println!("  name     : {}", token.name().await?);
    println!("  symbol   : {}", token.symbol().await?);
    println!("  decimals : {}", token.decimals().await?);
    println!("  supply   : {}", token.total_supply().await?);

    // ── Approach 2: ContractInstance with raw calldata ────────────────────────
    //
    // Manually encode the `name()` selector and call `.call().await`.

    let name_calldata: tronz::primitives::Bytes = ITRC20::nameCall {}.abi_encode().into();
    let iface = Interface::empty();
    let instance = provider.contract(contract, iface);

    let output = instance.call_raw(name_calldata).call().await?;
    let name = ITRC20::nameCall::abi_decode_returns(&output)?;
    println!("\n=== Raw calldata + manual decode ===");
    println!("  name (decoded) : {name}");

    // ── Approach 3: Interface + JSON ABI (dynamic) ────────────────────────────
    //
    // Build an `Interface` from a JSON ABI string and call by function name.
    // Useful when you don't have compile-time `sol!` bindings.

    let abi_json = r#"[
        {"name":"decimals","type":"function","inputs":[],"outputs":[{"type":"uint8"}],"stateMutability":"view"},
        {"name":"totalSupply","type":"function","inputs":[],"outputs":[{"type":"uint256"}],"stateMutability":"view"}
    ]"#;

    let abi: JsonAbi = serde_json::from_str(abi_json)?;
    let interface = Interface::new(abi);
    let dyn_instance = provider.contract(contract, interface);

    println!("\n=== Dynamic ABI call ===");

    let decimals_vals = dyn_instance.call("decimals", &[]).await?;
    println!("  decimals   : {:?}", decimals_vals.first());

    let supply_vals = dyn_instance.call("totalSupply", &[]).await?;
    if let Some(DynSolValue::Uint(supply, _)) = supply_vals.first() {
        println!("  totalSupply: {supply}");
    }

    Ok(())
}
