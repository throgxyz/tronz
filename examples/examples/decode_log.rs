//! Decode raw event logs from a receipt into typed `SolEvent` structs.
//!
//! Demonstrates two log-decoding approaches:
//!
//! 1. **Static** — `sol!`-generated type + `decode_logs::<E>()` helper
//! 2. **Dynamic** — `Interface::decode_logs()` when the ABI is only known at runtime
//!
//! No private key required (read-only).
//!
//! Required env:
//!   TRON_TX_ID    — hex transaction id that emitted TRC20 Transfer events
//!   TRON_CONTRACT — TRC20 contract address (for the dynamic ABI approach)
//!
//! Optional env:
//!   TRON_API_KEY  — TronGrid API key
//!
//! ```bash
//! TRON_TX_ID=<txid> TRON_CONTRACT=<contract> \
//!   cargo run -p examples --example decode_log
//! ```

use alloy_json_abi::JsonAbi;
use tronz::{
    ProviderBuilder, TRONGRID_NILE, TronProvider,
    contract::{ContractExt, Interface, SolEvent, decode_logs, trc20::ITRC20},
    primitives::B256,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tx_id_hex = std::env::var("TRON_TX_ID").expect("TRON_TX_ID env var required");
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let tx_id_bytes = hex::decode(tx_id_hex.trim_start_matches("0x"))?;
    let tx_id: B256 = B256::from_slice(&tx_id_bytes);
    let contract: tronz::Address = contract_str.parse()?;

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Fetch receipt ─────────────────────────────────────────────────────────

    let info = provider.get_transaction_info(tx_id).await?;
    println!("=== Transaction 0x{} ===", hex::encode(tx_id));
    println!("  status : {:?}", info.status);
    println!("  logs   : {}", info.logs.len());

    // ── Approach 1: Static decode via sol!-generated type ────────────────────

    println!("\n=== Static decode (ITRC20::Transfer) ===");
    println!(
        "  Transfer topic0 : 0x{}",
        hex::encode(ITRC20::Transfer::SIGNATURE_HASH)
    );

    let transfers: Vec<_> =
        decode_logs::<ITRC20::Transfer>(&info.logs).collect::<Result<_, _>>()?;

    if transfers.is_empty() {
        println!("  no Transfer events found");
    } else {
        for (i, t) in transfers.iter().enumerate() {
            let from: tronz::Address = t.from.into();
            let to: tronz::Address = t.to.into();
            println!("  [{}] from={from} to={to} value={}", i, t.value);
        }
    }

    // Also decode Approval events.
    let approvals: Vec<_> =
        decode_logs::<ITRC20::Approval>(&info.logs).collect::<Result<_, _>>()?;

    if !approvals.is_empty() {
        println!("\n  Approval events:");
        for (i, a) in approvals.iter().enumerate() {
            let owner: tronz::Address = a.owner.into();
            let spender: tronz::Address = a.spender.into();
            println!(
                "  [{}] owner={owner} spender={spender} value={}",
                i, a.value
            );
        }
    }

    // ── Approach 2: Dynamic decode via Interface ───────────────────────────────

    println!("\n=== Dynamic decode (Interface from JSON ABI) ===");

    // Load the ABI for the contract (could come from a file, API, etc.)
    let abi_json = r#"[
        {"name":"Transfer","type":"event","inputs":[{"name":"from","type":"address","indexed":true},{"name":"to","type":"address","indexed":true},{"name":"value","type":"uint256","indexed":false}],"anonymous":false},
        {"name":"Approval","type":"event","inputs":[{"name":"owner","type":"address","indexed":true},{"name":"spender","type":"address","indexed":true},{"name":"value","type":"uint256","indexed":false}],"anonymous":false}
    ]"#;
    let abi: JsonAbi = serde_json::from_str(abi_json)?;
    let interface = Interface::new(abi);

    // Bind to the contract so decode_log knows which contract emitted each log.
    let instance = provider.contract(contract, interface);

    for (i, result) in instance.decode_logs(&info.logs).enumerate() {
        match result {
            Ok((name, decoded)) => {
                println!("  [{}] event={}", i, name);
                for val in &decoded.indexed {
                    println!("       indexed : {val:?}");
                }
                for val in &decoded.body {
                    println!("       body    : {val:?}");
                }
            }
            Err(e) => {
                println!("  [{}] decode error: {e}", i);
            }
        }
    }

    Ok(())
}
