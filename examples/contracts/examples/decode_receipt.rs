//! Fetch a known transaction by id and print the full receipt.
//!
//! Demonstrates reading back a confirmed transaction's receipt: status, energy
//! usage, bandwidth usage, contract result, and emitted logs.
//!
//! No private key required (read-only).
//!
//! Required env:
//!   TRON_TX_ID  — hex transaction id to look up (no 0x prefix)
//!
//! Optional env:
//!   TRON_API_KEY — TronGrid API key
//!
//! ```bash
//! TRON_TX_ID=<txid> cargo run -p examples-contracts --example decode_receipt
//! ```

use tronz::{ProviderBuilder, TRONGRID_NILE, TronProvider, primitives::B256};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tx_id_hex = std::env::var("TRON_TX_ID").expect("TRON_TX_ID env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let tx_id_bytes = hex::decode(tx_id_hex.trim_start_matches("0x"))?;
    let tx_id: B256 = B256::from_slice(&tx_id_bytes);

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Fetch transaction ─────────────────────────────────────────────────────

    let signed = provider.get_transaction(tx_id).await?;
    println!("=== Transaction ===");
    println!("  tx_id       : 0x{}", hex::encode(tx_id));
    println!("  signatures  : {}", signed.signatures.len());
    println!("  expiration  : {} ms", signed.raw.expiration);
    println!("  timestamp   : {} ms", signed.raw.timestamp);

    // ── Fetch receipt ─────────────────────────────────────────────────────────

    let info = provider.get_transaction_info(tx_id).await?;
    println!("\n=== Receipt ===");
    println!("  block       : #{}", info.block_number);
    println!("  block ts    : {} ms", info.block_timestamp);
    println!("  status      : {:?}", info.status);
    println!("  contract    : {:?}", info.contract_result);

    println!("\n=== Resource usage ===");
    println!("  energy used : {}", info.energy_usage);
    println!("  energy fee  : {} sun", info.energy_fee.as_sun());
    println!("  net used    : {}", info.net_usage);
    println!("  net fee     : {} sun", info.net_fee.as_sun());

    if let Some(ref reason) = info.revert_reason {
        println!("\n=== Revert reason ===");
        println!("  {reason}");
    }

    if let Some(addr) = info.contract_address {
        println!("\n=== Deployed contract ===");
        println!("  address : {addr}");
    }

    // ── Logs ─────────────────────────────────────────────────────────────────

    if info.logs.is_empty() {
        println!("\n=== Logs: none ===");
    } else {
        println!("\n=== Logs ({}) ===", info.logs.len());
        for (i, log) in info.logs.iter().enumerate() {
            println!("  [{}] address : {}", i, log.address);
            for (j, topic) in log.topics.iter().enumerate() {
                println!("       topic[{j}] : 0x{}", hex::encode(topic));
            }
            println!("       data    : 0x{}", hex::encode(&log.data));
        }
    }

    Ok(())
}
