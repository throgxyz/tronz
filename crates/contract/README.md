# tronz-contract

ABI bindings and contract instances for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

TRON smart contracts are EVM-compatible, so this crate reuses `alloy`'s
[`sol!`](https://docs.rs/alloy-sol-macro) macro and ABI codec directly rather than
hand-rolling an encoder.

## Features

| Feature | What it enables |
|---------|-----------------|
| *(none)* | Static ABI encode/decode via `sol!` (no I/O, no provider dep) |
| `provider` | [`ContractInstance`], [`Interface`], [`Trc20Instance`], and extension traits |

## Interacting with arbitrary contracts (dynamic ABI)

Load a JSON ABI at runtime and call any function by name:

```rust,ignore
use tronz_contract::{Interface, JsonAbi, instance::ContractExt};

let abi: JsonAbi = serde_json::from_str(ABI_JSON).unwrap();
let contract = provider.contract(address, abi.into());

// read-only call
let values = contract.call("balanceOf", &[account.into()]).await?;

// state-changing call
let pending = contract.send("transfer", &[to.into(), amount.into()]).await?;
let receipt = pending.await_success().await?;
```

## Deploying with ABI metadata

`DeployBuilder` accepts Alloy's typed `JsonAbi` and converts it to native
`TronAbi` metadata before sending the protobuf request:

```rust,ignore
use tronz_contract::{ContractExt as _, JsonAbi};

let abi: JsonAbi = serde_json::from_str(ABI_JSON)?;
let pending = provider
    .deploy(bytecode)
    .abi(abi)
    .name("MyContract")
    .send()
    .await?;
```

Provider queries return native `TronAbi` so all node metadata remains readable,
including unknown entry types and incomplete tuples. Convert explicitly when a
dynamic Alloy interface is needed:

```rust,ignore
let info = provider.get_contract_info(address).await?;
let json_abi = info.abi.try_to_json_abi()?;
let contract = provider.contract(address, Interface::new(json_abi));
```

Use `.tron_abi(abi)` instead of `.abi(abi)` to deploy already-native metadata
without an Alloy conversion.

## Standard token interfaces (static ABI)

Use the typed wrappers for well-known standards:

```rust,ignore
use tronz_contract::trc20::Trc20Ext;

let token = provider.trc20(usdt_address);
println!("name    : {}", token.name().await?);
println!("balance : {}", token.balance_of(my_address).await?);

let pending = token.transfer(recipient, amount).await?;
let receipt = pending.await_success().await?;
```

## Crate layout

- [`trc20`] — static `sol!` bindings + [`Trc20Instance`] high-level wrapper
- [`Interface`] wrapping [`JsonAbi`] with O(1) selector lookup
- [`ContractInstance`] — generic contract handle
- [`ContractError`] and [`Result`] type alias

[`ContractInstance`]: crate::ContractInstance
[`Interface`]: crate::Interface
[`Trc20Instance`]: crate::trc20::Trc20Instance
[`ContractError`]: crate::ContractError
[`Result`]: crate::Result
[`trc20`]: crate::trc20
[`JsonAbi`]: alloy_json_abi::JsonAbi

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
