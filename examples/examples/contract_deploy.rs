//! Deploy a compiled contract from bytecode.
//!
//! Deploys a minimal Solidity contract to the Nile testnet and extracts the
//! deployed contract address from the receipt.
//!
//! Required env:
//!   TRON_PRIVATE_KEY — funded Nile private key
//!
//! Optional env:
//!   TRON_API_KEY     — TronGrid API key
//!
//! ```bash
//! TRON_PRIVATE_KEY=<key> cargo run -p examples --example contract_deploy
//! ```

use tronz::{
    LocalSigner, ProviderBuilder, TRONGRID_NILE, TronProvider, TronSigner, Trx,
    contract::ContractExt,
};

// Hand-assembled EVM bytecode for a minimal Counter contract:
//
// ```solidity
// contract Counter {
//     uint256 public count;           // slot 0
//     function increment() external { count += 1; }
// }
// ```
//
// Selectors (first 4 bytes of keccak256 of signature):
//   count()      → 0x06661abd
//   increment()  → 0x7cf5dab0
//
// Layout (64 bytes total):
//   [0x00–0x0a] constructor  – CODECOPY(0, 0x0b, 0x35) then RETURN 0x35 bytes
//   [0x0b–0x3f] runtime      – dispatcher + count() + increment()
const COUNTER_BYTECODE: &str = concat!(
    // ── Constructor (11 bytes) ────────────────────────────────────────────────
    // PUSH1 0x35   DUP1   PUSH1 0x0b   PUSH1 0x00   CODECOPY   PUSH1 0x00   RETURN
    "603580600b6000396000f3",
    // ── Runtime (53 bytes) ───────────────────────────────────────────────────
    // Dispatcher: calldataload >> 0xe0 → selector
    "6000", // PUSH1 0  (calldataload offset)
    "35",   // CALLDATALOAD
    "60e0", // PUSH1 0xe0
    "1c",   // SHR  → top 4 bytes = selector
    "80",   // DUP1
    // count() branch
    "6306661abd", // PUSH4 count() selector
    "14",         // EQ
    "601e",       // PUSH1 0x1e = 30  (count() JUMPDEST)
    "57",         // JUMPI
    "80",         // DUP1
    // increment() branch
    "637cf5dab0", // PUSH4 increment() selector
    "14",         // EQ
    "602a",       // PUSH1 0x2a = 42  (increment() JUMPDEST)
    "57",         // JUMPI
    // fallback: revert
    "6000", // PUSH1 0
    "80",   // DUP1
    "fd",   // REVERT
    // count() at PC=30 (0x1e)
    "5b",   // JUMPDEST
    "6000", // PUSH1 0  (storage slot 0)
    "54",   // SLOAD
    "6000", // PUSH1 0  (memory offset)
    "52",   // MSTORE
    "6020", // PUSH1 32 (return size)
    "6000", // PUSH1 0  (return offset)
    "f3",   // RETURN
    // increment() at PC=42 (0x2a)
    "5b",   // JUMPDEST
    "6001", // PUSH1 1
    "6000", // PUSH1 0  (slot 0)
    "54",   // SLOAD
    "01",   // ADD
    "6000", // PUSH1 0  (slot 0)
    "55",   // SSTORE
    "00",   // STOP
);

// JSON ABI for the Counter contract.
const COUNTER_ABI: &str = r#"[
    {"name":"count","type":"function","inputs":[],"outputs":[{"type":"uint256"}],"stateMutability":"view"},
    {"name":"increment","type":"function","inputs":[],"outputs":[],"stateMutability":"nonpayable"}
]"#;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key_hex = std::env::var("TRON_PRIVATE_KEY").expect("TRON_PRIVATE_KEY env var required");
    let api_key = std::env::var("TRON_API_KEY").ok();

    let signer = LocalSigner::from_hex(&key_hex)?;
    let deployer = signer.address();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .maybe_api_key(api_key)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let balance = provider.get_account(deployer).await?.balance;
    println!("=== Deployer {} ===", deployer);
    println!("  balance : {} TRX", balance.as_trx());

    // Deployment costs energy; make sure there's enough TRX for the fee_limit.
    if balance < Trx::from_sun(5_000_000)? {
        anyhow::bail!("balance too low — need at least 5 TRX for deployment");
    }

    // ── Deploy ────────────────────────────────────────────────────────────────

    let bytecode = hex::decode(COUNTER_BYTECODE)?;

    println!("\n=== Deploying Counter contract ===");
    println!("  bytecode size : {} bytes", bytecode.len());

    let pending = provider
        .deploy(bytecode)
        .abi(COUNTER_ABI)
        .name("Counter")
        .fee_limit(Trx::from_sun(20_000_000)?) // 20 TRX fee limit
        .send()
        .await?;

    println!("  tx_id  : 0x{}", hex::encode(pending.tx_id()));
    println!("  waiting for confirmation…");
    let info = pending.get_receipt().await?;

    println!("\n=== Deployment receipt ===");
    println!("  status          : {:?}", info.status);
    println!("  energy used     : {}", info.energy_usage);
    println!("  energy fee      : {} sun", info.energy_fee.as_sun());

    let contract_addr = info.contract_address.ok_or_else(|| {
        anyhow::anyhow!("no contract address in receipt — deployment may have failed")
    })?;

    println!("\n=== Deployed contract ===");
    println!("  address : {contract_addr}");
    println!("  explorer: https://nile.tronscan.org/#/contract/{contract_addr}");

    // ── Verify by calling count() ─────────────────────────────────────────────

    use alloy_dyn_abi::DynSolValue;
    use alloy_json_abi::JsonAbi;
    use tronz::contract::Interface;

    let abi: JsonAbi = serde_json::from_str(COUNTER_ABI)?;
    let interface = Interface::new(abi);
    let instance = provider.contract(contract_addr, interface);

    let vals = instance.call("count", &[]).await?;
    let count = vals.first().and_then(|v| {
        if let DynSolValue::Uint(n, _) = v {
            Some(*n)
        } else {
            None
        }
    });
    println!("\n=== Initial state ===");
    println!("  count() = {:?}", count);

    Ok(())
}
