//! Estimate energy consumption before executing a contract call.
//!
//! `estimate_energy` simulates the call and returns the energy it would
//! consume. Use this to set an appropriate `fee_limit` before sending.
//!
//! Energy cost in TRX = `energy_used × dynamic_energy_price / 1_000_000`
//!
//! No private key required for the estimate itself. A signer is optional but
//! improves accuracy (the simulation uses the signer's address as caller).
//!
//! Required env:
//!   TRON_CONTRACT — TRC20 contract address to simulate a call against
//!
//! Optional env:
//!   TRON_API_KEY  — TronGrid API key
//!   TRON_ADDRESS  — caller address for the simulation (defaults to contract addr)
//!
//! ```bash
//! TRON_CONTRACT=<addr> cargo run -p examples --example contract_estimate_energy
//! ```

use tronz::{
    ProviderBuilder, TRONGRID_NILE, TronProvider, U256,
    contract::{ContractExt, Interface, SolCall, trc20::ITRC20},
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

    // ── Estimate energy for `balanceOf` ───────────────────────────────────────
    //
    // `balanceOf` is a view function — it reads state but doesn't modify it.
    // The energy estimate is usually small (~700 energy units for USDT).

    let calldata: tronz::primitives::Bytes = ITRC20::balanceOfCall {
        account: contract.into(), // query the contract's own balance
    }
    .abi_encode()
    .into();

    let instance = provider.contract(contract, Interface::empty());
    let energy_estimate = instance.call_raw(calldata).estimate_energy().await?;
    println!("=== Energy estimate ===");
    println!("  function   : balanceOf(address)");
    println!("  energy     : {energy_estimate} units");

    // Convert to approximate TRX cost.
    // Dynamic energy price varies; 420 sun/energy is a typical ballpark.
    let approx_sun = energy_estimate * 420;
    println!(
        "  approx fee : {} sun  (~{:.6} TRX at 420 sun/energy)",
        approx_sun,
        approx_sun as f64 / 1_000_000.0
    );

    // ── Estimate energy for `transfer` ────────────────────────────────────────
    //
    // `transfer` is a state-changing function and costs more energy.
    // The estimate uses the current state; actual cost may differ slightly.

    let transfer_calldata: tronz::primitives::Bytes = ITRC20::transferCall {
        to: contract.into(), // dummy: transfer to self
        amount: U256::from(1u64),
    }
    .abi_encode()
    .into();

    let transfer_energy = instance
        .call_raw(transfer_calldata)
        .estimate_energy()
        .await?;
    println!("\n  function   : transfer(address,uint256)");
    println!("  energy     : {transfer_energy} units");

    let approx_sun2 = transfer_energy * 420;
    println!(
        "  approx fee : {} sun  (~{:.6} TRX at 420 sun/energy)",
        approx_sun2,
        approx_sun2 as f64 / 1_000_000.0
    );

    // ── Chain parameters ──────────────────────────────────────────────────────
    //
    // The actual energy price is stored in chain parameters.

    let params = provider.chain_parameters().await?;
    if let Some(price) = params.get("getEnergyFee") {
        println!("\n=== Current energy price ===");
        println!("  getEnergyFee  : {price} sun/energy");
        println!(
            "  transfer cost : {} sun  ({:.6} TRX)",
            transfer_energy * price,
            transfer_energy as f64 * *price as f64 / 1_000_000.0
        );
    }

    Ok(())
}
