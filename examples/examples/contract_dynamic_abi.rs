//! Use `Interface` with a JSON ABI string for runtime encoding and decoding.
//!
//! When you don't have compile-time `sol!` bindings (e.g. the ABI is loaded
//! from a file or fetched from a block explorer), use `Interface` for dynamic
//! function encoding, decoding, and log decoding.
//!
//! This example loads the USDT ABI as a JSON string, builds an `Interface`,
//! and calls `name()`, `decimals()`, and `balanceOf(address)`.
//!
//! No private key required (read-only).
//!
//! Required env:
//!   TRON_CONTRACT — TRC20 contract address
//!   TRON_ADDRESS  — address to query balance of
//!
//! Optional env:
//!   TRON_API_KEY  — TronGrid API key
//!
//! ```bash
//! TRON_CONTRACT=TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t \
//! TRON_ADDRESS=TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t \
//!   cargo run -p examples --example contract_dynamic_abi
//! ```

use alloy_dyn_abi::DynSolValue;
use alloy_json_abi::JsonAbi;
use tronz::{
    ProviderBuilder, TRONGRID_NILE,
    contract::{ContractExt, Interface},
};

// Minimal TRC20 ABI — a subset of the full ERC-20 interface.
const ERC20_ABI: &str = r#"[
    {
        "name": "name",
        "type": "function",
        "inputs": [],
        "outputs": [{"name": "", "type": "string"}],
        "stateMutability": "view"
    },
    {
        "name": "symbol",
        "type": "function",
        "inputs": [],
        "outputs": [{"name": "", "type": "string"}],
        "stateMutability": "view"
    },
    {
        "name": "decimals",
        "type": "function",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint8"}],
        "stateMutability": "view"
    },
    {
        "name": "totalSupply",
        "type": "function",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view"
    },
    {
        "name": "balanceOf",
        "type": "function",
        "inputs": [{"name": "account", "type": "address"}],
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view"
    },
    {
        "name": "Transfer",
        "type": "event",
        "inputs": [
            {"name": "from",  "type": "address", "indexed": true},
            {"name": "to",    "type": "address", "indexed": true},
            {"name": "value", "type": "uint256",  "indexed": false}
        ],
        "anonymous": false
    }
]"#;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let addr_str = std::env::var("TRON_ADDRESS").expect("TRON_ADDRESS env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let contract: tronz::Address = contract_str.parse()?;
    let addr: tronz::Address = addr_str.parse()?;

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Build Interface ───────────────────────────────────────────────────────

    let abi: JsonAbi = serde_json::from_str(ERC20_ABI)?;
    let interface = Interface::new(abi);

    println!("=== Interface ===");
    println!(
        "  functions : {:?}",
        interface.abi().functions.keys().collect::<Vec<_>>()
    );
    println!(
        "  events    : {:?}",
        interface.abi().events.keys().collect::<Vec<_>>()
    );

    // Bind the interface to the contract address.
    let instance = provider.contract(contract, interface);

    // ── No-arg calls ──────────────────────────────────────────────────────────

    let name = dyn_string(instance.call("name", &[]).await?);
    let symbol = dyn_string(instance.call("symbol", &[]).await?);
    let decimals = dyn_uint8(instance.call("decimals", &[]).await?);
    let supply = dyn_uint256(instance.call("totalSupply", &[]).await?);

    println!("\n=== Token metadata ===");
    println!("  name        : {name}");
    println!("  symbol      : {symbol}");
    println!("  decimals    : {decimals}");
    println!("  totalSupply : {supply}");

    // ── Call with arguments ───────────────────────────────────────────────────
    //
    // `DynSolValue::Address` expects a 20-byte alloy address (no 0x41 prefix).

    let account_arg = DynSolValue::Address(addr.into());
    let balance_vals = instance.call("balanceOf", &[account_arg]).await?;
    let balance = dyn_uint256(balance_vals);

    println!("\n=== Balance of {} ===", addr);
    println!("  raw units : {balance}");
    if decimals > 0 {
        let divisor = 10u128.pow(decimals.into());
        println!(
            "  display   : {:.6} {symbol}",
            balance as f64 / divisor as f64
        );
    }

    Ok(())
}

fn dyn_string(vals: Vec<DynSolValue>) -> String {
    match vals.into_iter().next() {
        Some(DynSolValue::String(s)) => s,
        _ => "<unknown>".to_owned(),
    }
}

fn dyn_uint8(vals: Vec<DynSolValue>) -> u8 {
    match vals.into_iter().next() {
        Some(DynSolValue::Uint(n, _)) => n.try_into().unwrap_or(0),
        _ => 0,
    }
}

fn dyn_uint256(vals: Vec<DynSolValue>) -> u128 {
    match vals.into_iter().next() {
        Some(DynSolValue::Uint(n, _)) => n.try_into().unwrap_or(0),
        _ => 0,
    }
}
