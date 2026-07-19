# tronz-contract

ABI bindings, typed contract instances, deployment, and event filtering for the
[tronz](https://github.com/throgxyz/tronz) TRON SDK.

TRON smart contracts are EVM-compatible, so this crate reuses `alloy`'s ABI
codec and provides `tron_sol!` for generating provider-bound TRON contract
bindings from Solidity syntax or a JSON ABI.

## Features

| Feature | What it enables |
|---------|-----------------|
| *(none)* | Static ABI encode/decode and `sol!` type generation (no provider dependency) |
| `provider` | Provider-bound `tron_sol!` instances, [`ContractInstance`], [`Trc20Instance`], [`Trc721Instance`], call/deploy builders, and [`TronEventFilter`] |

## Interacting with arbitrary contracts (dynamic ABI)

Load a JSON ABI at runtime and call any function by name:

```rust,ignore
use tronz_contract::{Interface, JsonAbi, instance::ContractExt};

let abi: JsonAbi = serde_json::from_str(ABI_JSON).unwrap();
let contract = provider.contract(address, abi.into()).caller(account);

// read-only call
let values = contract.call("balanceOf", &[account.into()]).await?;

// state-changing call
let pending = contract.send("transfer", &[to.into(), amount.into()]).await?;
let receipt = pending.get_receipt().await?;
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

let token = provider.trc20(usdt_address).caller(my_address);
println!("name    : {}", token.name().await?);
println!("balance : {}", token.balance_of(my_address).await?);

let pending = token.transfer(recipient, amount).await?;
let receipt = pending.get_receipt().await?;
```

`Trc721Instance` provides the equivalent typed interface for NFT metadata,
ownership, transfers, approvals, and operators:

```rust,ignore
use tronz_contract::trc721::Trc721Ext;

let nft = provider.trc721(contract_address).caller(my_address);
let owner = nft.owner_of(token_id).await?;
```

### Reading solidified contract state

The same contract bindings accept a read-only `SolidityProvider`. Set a caller
when no signer-backed FullNode provider is attached so contracts that inspect
`msg.sender` execute with the intended address:

```rust,ignore
use tronz_contract::trc20::Trc20Ext;

let token = solidity_provider.trc20(usdt_address).caller(my_address);
let balance = token.balance_of(my_address).await?;
```

Constant calls, energy estimation, and event queries are available over either
provider. Sending and deploying still require a signer-backed `TronProvider`.

## Generating provider-bound bindings

`tron_sol!` accepts Solidity syntax or a JSON ABI path and generates typed call
and event builders bound to a TRON provider:

```rust,ignore
use tronz_contract::tron_sol;

tron_sol! {
    #[sol(rpc)]
    interface IToken {
        function balanceOf(address owner) external view returns (uint256);
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}

let token = IToken::new(contract_address, provider).caller(my_address);
let balance = token.balance_of(owner).call().await?;
let transfers = token.Transfer_filter().query_block(block_number).await?;
```

## Crate layout

- [`trc20`] — static bindings and the [`Trc20Instance`] high-level wrapper
- [`trc721`] — static bindings and the [`Trc721Instance`] high-level wrapper
- [`tron_sol!`] — provider-bound typed calls and [`TronEventFilter`] builders
- [`DeployBuilder`] — contract deployment with native or Alloy ABI metadata
- [`Interface`] wrapping [`JsonAbi`] with O(1) selector lookup
- [`ContractInstance`] — generic contract handle
- [`ContractError`] and [`Result`] type alias

[`ContractInstance`]: crate::ContractInstance
[`Interface`]: crate::Interface
[`Trc20Instance`]: crate::trc20::Trc20Instance
[`Trc721Instance`]: crate::trc721::Trc721Instance
[`TronEventFilter`]: crate::TronEventFilter
[`DeployBuilder`]: crate::DeployBuilder
[`tron_sol!`]: crate::tron_sol
[`ContractError`]: crate::ContractError
[`Result`]: crate::Result
[`trc20`]: crate::trc20
[`trc721`]: crate::trc721
[`JsonAbi`]: alloy_json_abi::JsonAbi

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
