//! Trigger a known-revert function and decode the ABI-encoded revert reason.
//!
//! When a Solidity function reverts with `revert("reason")` or a custom error,
//! the revert data is ABI-encoded and returned. tronz surfaces the decoded
//! string in `TransactionInfo::revert_reason`.
//!
//! This example demonstrates how to:
//! 1. Detect a revert from a constant call (`ContractError::ContractRevert`)
//! 2. Detect a revert from a broadcast call (`TransactionInfo::contract_result`)
//! 3. Decode a raw ABI-encoded revert reason manually
//!
//! No private key required for constant calls.
//!
//! Required env:
//!   TRON_CONTRACT — address of a contract known to have a reverting function
//!
//! Optional env:
//!   TRON_API_KEY  — TronGrid API key
//!
//! ```bash
//! TRON_CONTRACT=<addr> cargo run -p examples --example contract_revert
//! ```

use tronz::{
    ProviderBuilder, TRONGRID_NILE,
    contract::{ContractError, ContractExt, Interface, SolCall, SolValue, trc20::ITRC20},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let contract_str = std::env::var("TRON_CONTRACT").expect("TRON_CONTRACT env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let contract: tronz::Address = contract_str.parse()?;

    let provider = ProviderBuilder::new()
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // ── Trigger a revert via constant call ────────────────────────────────────
    //
    // Calling `transfer` as a constant call: a zero-value transfer to address(0)
    // will typically revert on a real ERC-20 (invalid recipient). The node
    // executes the function and returns the revert data without broadcasting.

    let calldata: tronz::primitives::Bytes = ITRC20::transferCall {
        to: alloy_primitives::Address::ZERO,
        amount: tronz::U256::ZERO,
    }
    .abi_encode()
    .into();

    let instance = provider.contract(contract, Interface::empty());
    let result = instance.call_raw(calldata).call().await;

    // Helper: check and decode an Error(string) revert payload.
    // The ABI selector for `Error(string)` is `0x08c379a0`.
    let decode_revert = |data: &[u8]| {
        if data.len() >= 4 && data[..4] == [0x08, 0xc3, 0x79, 0xa0] {
            match String::abi_decode(&data[4..]) {
                Ok(msg) => println!("  decoded reason: {:?}", msg),
                Err(e) => println!("  decode failed: {e}"),
            }
        }
    };

    println!("=== Constant call result ===");
    match result {
        // TRON's trigger_constant_contract returns revert data as Ok(output)
        // rather than as an error — check for the Error(string) selector.
        Ok(output) if output.len() >= 4 && output[..4] == [0x08, 0xc3, 0x79, 0xa0] => {
            println!("  reverted! raw data: 0x{}", hex::encode(&output));
            decode_revert(&output);
        }
        Ok(output) => {
            println!("  succeeded — output: 0x{}", hex::encode(&output));
        }
        Err(ContractError::ContractRevert(data)) => {
            println!("  reverted! raw data: 0x{}", hex::encode(&data));
            decode_revert(&data);
        }
        Err(e) => {
            println!("  error (not a revert): {e}");
        }
    }

    println!();
    println!("Note: inspect `TransactionInfo::revert_reason` from get_receipt()");
    println!("for broadcast calls — tronz pre-decodes the standard Error(string).");

    Ok(())
}
