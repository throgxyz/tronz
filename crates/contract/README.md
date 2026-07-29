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

```rust,no_run
use tronz_contract::{ContractExt as _, Interface, JsonAbi};

# use alloy_dyn_abi::DynSolValue;
# use tronz_primitives::{Address, U256};
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     abi: JsonAbi,
#     address: Address,
#     account: Address,
#     to: Address,
#     amount: U256,
# ) -> Result<(), Box<dyn std::error::Error>> {
let contract = provider.contract(address, Interface::new(abi)).caller(account);

// read-only call
let values = contract.call("balanceOf", &[DynSolValue::Address(account.into())]).await?;

// state-changing call
let args = [DynSolValue::Address(to.into()), DynSolValue::Uint(amount, 256)];
let pending = contract.send("transfer", &args).await?;
let receipt = pending.get_receipt().await?;
# Ok(()) }
```

## Deploying with ABI metadata

`DeployBuilder` accepts Alloy's typed `JsonAbi` and converts it to native
`TronAbi` metadata before sending the protobuf request:

```rust,no_run
use tronz_contract::{ContractExt as _, JsonAbi};

# use tronz_primitives::Bytes;
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     abi: JsonAbi,
#     bytecode: Bytes,
# ) -> Result<(), Box<dyn std::error::Error>> {
let pending = provider.deploy(bytecode).abi(abi).name("MyContract").send().await?;
# Ok(()) }
```

Provider queries return native `TronAbi` so all node metadata remains readable,
including unknown entry types and incomplete tuples. Convert explicitly when a
dynamic Alloy interface is needed:

```rust,no_run
# use tronz_contract::{ContractExt as _, Interface};
# use tronz_primitives::Address;
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     address: Address,
# ) -> Result<(), Box<dyn std::error::Error>> {
let info = provider.get_contract_info(address).await?;
let json_abi = info.abi.try_to_json_abi()?;
let contract = provider.contract(address, Interface::new(json_abi));
# Ok(()) }
```

Use `.tron_abi(abi)` instead of `.abi(abi)` to deploy already-native metadata
without an Alloy conversion.

## Multisig calls and deployments

Contract calls and deployments expose the same unsigned transaction flow as
provider builders. Set the multisig account and active permission, then call
`.build()` to collect the required signatures before broadcasting:

```rust,no_run
# use tronz_contract::ContractInstance;
# use tronz_primitives::{Address, Bytes};
# use tronz_provider::{TronProvider, types::SignedTransaction};
# use tronz_signer::{TronNetworkWallet, TronWallet};
# async fn run(
#     provider: impl TronProvider + Clone,
#     contract: ContractInstance<impl TronProvider + Clone>,
#     calldata: Bytes,
#     multisig_account: Address,
#     wallet: TronWallet,
#     keys: Vec<Address>,
# ) -> Result<(), Box<dyn std::error::Error>> {
let raw = contract
    .call_raw(calldata)
    .caller(multisig_account)
    .permission_id(2)
    .build()
    .await?;

let signatures = wallet.sign_hash_with_many(&keys, &raw.tx_id()).await?;
let signed = SignedTransaction { raw, signatures };
provider.broadcast(signed).await?;
# Ok(()) }
```

For deployments, use `.from(multisig_account).permission_id(2).build()`.
`.send()` remains the single-signature convenience path.

## Standard token interfaces (static ABI)

Use the typed wrappers for well-known standards:

```rust,no_run
use tronz_contract::trc20::Trc20Ext;

# use tronz_primitives::{Address, U256};
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     usdt_address: Address,
#     my_address: Address,
#     recipient: Address,
#     amount: U256,
# ) -> Result<(), Box<dyn std::error::Error>> {
let token = provider.trc20(usdt_address).caller(my_address);
println!("name    : {}", token.name().await?);
println!("balance : {}", token.balance_of(my_address).await?);

let pending = token.transfer(recipient, amount).await?;
let receipt = pending.get_receipt().await?;
# Ok(()) }
```

Every convenience method also has a typed `*_call` form when the transaction
needs configuration or an unsigned build:

```rust,no_run
# use tronz_contract::trc20::Trc20Ext;
# use tronz_primitives::{Address, U256, parse_trx};
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     usdt_address: Address,
#     recipient: Address,
#     amount: U256,
# ) -> Result<(), Box<dyn std::error::Error>> {
# let token = provider.trc20(usdt_address);
let raw = token
    .transfer_call(recipient, amount)
    .fee_limit(parse_trx("100")?)
    .permission_id(2)
    .build()
    .await?;
# Ok(()) }
```

`Trc721Instance` provides the equivalent typed interface for NFT metadata,
ownership, transfers, approvals, and operators:

```rust,no_run
use tronz_contract::trc721::Trc721Ext;

# use tronz_primitives::{Address, U256};
# async fn run(
#     provider: impl tronz_provider::ContractReadProvider + Clone,
#     contract_address: Address,
#     my_address: Address,
#     token_id: U256,
# ) -> Result<(), Box<dyn std::error::Error>> {
let nft = provider.trc721(contract_address).caller(my_address);
let owner = nft.owner_of(token_id).await?;
# Ok(()) }
```

### Reading solidified contract state

The same contract bindings accept a read-only `SolidityProvider`. Set a caller
when no signer-backed FullNode provider is attached so contracts that inspect
`msg.sender` execute with the intended address:

```rust,no_run
use tronz_contract::trc20::Trc20Ext;

# use tronz_primitives::Address;
# use tronz_provider::{SolidityProvider, transport::SolidityTransport};
# async fn run(
#     solidity_provider: SolidityProvider,
#     usdt_address: Address,
#     my_address: Address,
# ) -> Result<(), Box<dyn std::error::Error>> {
let token = solidity_provider.trc20(usdt_address).caller(my_address);
let balance = token.balance_of(my_address).await?;
# Ok(()) }
```

Constant calls, energy estimation, and event queries are available over either
provider. Sending and deploying still require a signer-backed `TronProvider`.

## Generating provider-bound bindings

`tron_sol!` accepts Solidity syntax or a JSON ABI path and generates typed call
and event builders bound to a TRON provider:

```rust,no_run
use tronz_contract::tron_sol;

tron_sol! {
    #[sol(rpc)]
#   #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IToken {
        function balanceOf(address owner) external view returns (uint256);
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}

# use tronz_primitives::Address;
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     contract_address: Address,
#     my_address: Address,
#     owner: Address,
#     block_number: i64,
# ) -> Result<(), Box<dyn std::error::Error>> {
let token = IToken::new(contract_address, provider).caller(my_address);
let balance = token.balanceOf(owner).call().await?;
let transfers = token.Transfer_filter().query_block(block_number).await?;
# Ok(()) }
# fn main() {}
```

## Events

TRON has no `eth_getLogs` and no log subscription, so logs are read per
transaction or per block and filtered locally. `query_range` walks a block range
concurrently, and `watch` follows the chain by polling:

```rust,no_run
use futures::StreamExt;

# use tronz_contract::tron_sol;
# tron_sol! {
#     #[sol(rpc)]
#     #[tron_sol(tronz_crate = ::tronz_contract)]
#     interface IToken {
#         event Transfer(address indexed from, address indexed to, uint256 value);
#     }
# }
# use tronz_primitives::Address;
# async fn run(
#     provider: impl tronz_provider::TronProvider + Clone,
#     contract_address: Address,
#     from: i64,
#     to: i64,
# ) -> Result<(), Box<dyn std::error::Error>> {
# let token = IToken::new(contract_address, provider);
// Scan history, eight blocks at a time.
let past = token.Transfer_filter().concurrency(8).query_range(from, to).await?;

// Follow new transfers, trailing the head far enough to skip reverted blocks.
let mut stream = token.Transfer_filter().watch().await?.into_stream();
while let Some(transfer) = stream.next().await {
    let transfer = transfer?;
    println!("{} -> {}: {}", transfer.from, transfer.to, transfer.value);
}
# Ok(()) }
# fn main() {}
```

The watcher defaults to 19 confirmations, the point at which TRON solidifies a
block. Drive it with [`EventWatcher::poll`] instead of a stream to control the
pacing yourself or to persist [`EventWatcher::next_block`] and resume later.

Resuming from a stale block is safe: each poll advances at most
[`EventWatcher::max_blocks_per_poll`] blocks, so a long backlog is closed one
window at a time rather than in a single unbounded scan.

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
