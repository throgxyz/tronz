//! State-changing contract call — update state and poll receipt.
//!
//! `trigger_smart_contract` creates a real transaction that modifies on-chain
//! state, consumes energy, and must be signed and broadcast.
//!
//! This example calls `transfer(to, amount)` on a TRC20 contract using a raw
//! `ContractInstance` instead of `Trc20Instance` — to show how to wire up
//! any arbitrary function call.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — hex private key
//!   TRON_CONTRACT    — TRC20 contract address
//!   TRON_TO          — recipient address
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!   TRON_AMOUNT      — transfer amount in raw token units (default: 1)
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> TRON_CONTRACT=<addr> TRON_TO=<addr> \
//!   cargo run -p examples --example contract_send
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronSigner, U256,
    contract::{ContractExt, Interface, SolCall, trc20::ITRC20},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let to_str = std::env::var("TRON_TO").expect("TRON_TO env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();
    let amount = std::env::var("TRON_AMOUNT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(U256::from)
        .unwrap_or(U256::from(1u64));

    let signer = LocalSigner::from_hex(&key_hex)?;
    let from = signer.address();

    let contract: tronz::Address = contract_str.parse()?;
    let to: tronz::Address = to_str.parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    println!("=== Contract send ===");
    println!("  contract : {contract}");
    println!("  from     : {from}");
    println!("  to       : {to}");
    println!("  amount   : {amount}");

    // ── Encode calldata ───────────────────────────────────────────────────────
    //
    // `ITRC20::transferCall` is a `sol!`-generated type.
    // `.abi_encode()` produces the 4-byte selector + ABI-encoded arguments.
    // The address must be the EVM form (20 bytes) inside the ABI encoding —
    // tronz::Address's `Into<alloy_primitives::Address>` strips the 0x41 prefix.

    let calldata: tronz::primitives::Bytes = ITRC20::transferCall {
        to: to.into(),
        amount,
    }
    .abi_encode()
    .into();

    println!("  calldata : 0x{}", hex::encode(&calldata));

    // ── Send ──────────────────────────────────────────────────────────────────
    //
    // `ContractInstance::call_raw(calldata).send()` builds a
    // `TriggerSmartContract` request, fills TAPOS and fee_limit, signs,
    // and broadcasts.

    let instance = provider.contract(contract, Interface::empty());
    let pending = instance.call_raw(calldata).send().await?;
    println!("\n  tx_id  : 0x{}", hex::encode(pending.tx_id()));

    // ── Wait for confirmation ─────────────────────────────────────────────────

    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;
    println!("\n=== Receipt ===");
    println!("  status          : {:?}", info.status);
    println!("  contract result : {:?}", info.contract_result);
    println!("  energy used     : {}", info.energy_usage);
    println!("  energy fee      : {} sun", info.energy_fee.as_sun());
    if let Some(ref reason) = info.revert_reason {
        println!("  revert reason   : {reason}");
    }

    // ── Decode Transfer event from logs ───────────────────────────────────────

    use tronz::contract::decode_logs;
    let transfers: Vec<_> =
        decode_logs::<ITRC20::Transfer>(&info.logs).collect::<Result<_, _>>()?;

    if !transfers.is_empty() {
        println!("\n=== Transfer events ===");
        for t in &transfers {
            let f: tronz::Address = t.from.into();
            let t_addr: tronz::Address = t.to.into();
            println!("  from  : {f}");
            println!("  to    : {t_addr}");
            println!("  value : {}", t.value);
        }
    }

    Ok(())
}
