# Contracts

TRON smart contracts are EVM-compatible at the ABI layer. tronz reuses alloy
ABI crates for selectors, calldata, return values, events, and revert data.

## TRC20

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET};
use tronz::contract::Trc20Ext as _;

# async fn run() -> tronz::providers::Result<()> {
let provider = ProviderBuilder::new()
    .on_grpc(TRONGRID_MAINNET)
    .await?;

let usdt = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse()?;
let holder = usdt;
let token = provider.trc20(usdt);

let symbol = token.symbol().await?;
let balance = token.balance_of(holder).await?;
println!("{symbol}: {balance}");
# Ok(()) }
```

## Dynamic ABI

Use `Interface` when an ABI is loaded at runtime:

```rust,ignore
use alloy_dyn_abi::DynSolValue;
use alloy_json_abi::JsonAbi;
use tronz::contract::{ContractExt, Interface};

let abi: JsonAbi = serde_json::from_str(abi_json)?;
let contract = provider.contract(address, Interface::new(abi));
let values = contract.call("balanceOf", &[DynSolValue::Address(holder.into())]).await?;
```

## Sending And Deploying

State-changing contract calls require a signed provider with recommended
fillers. `FeeLimitFiller` supplies a default fee limit, and builders also allow
explicit `.fee_limit(...)` overrides where supported.

See:

- `cargo run -p examples-contracts --example contract_call`
- `cargo run -p examples-contracts --example contract_send`
- `cargo run -p examples-contracts --example contract_deploy`
- `cargo run -p examples-contracts --example contract_revert`
