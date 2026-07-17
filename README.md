# Tronz

An idiomatic, async-first Rust SDK for the [TRON](https://tron.network) network — inspired by [alloy](https://github.com/alloy-rs/alloy).

[![Crates.io](https://img.shields.io/crates/v/tronz.svg)](https://crates.io/crates/tronz)
[![docs.rs](https://docs.rs/tronz/badge.svg)](https://docs.rs/tronz)
[![License: MIT / Apache-2.0](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue.svg)](#license)
[![Rust 1.91.1+](https://img.shields.io/badge/rust-1.91.1%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/throgxyz/tronz/actions/workflows/ci.yml/badge.svg)](https://github.com/throgxyz/tronz/actions/workflows/ci.yml)

## Features

- **gRPC transport** — connects to TronGrid, FullNodes, and SolidityNodes via tonic
- **Solidified state** — read-only `SolidityProvider` queries irreversible blocks, accounts, transactions, and contract state
- **Resilient by default** — per-call timeouts plus automatic retries with exponential back-off and jitter, configurable via `ProviderBuilder` / `GrpcTransport::builder()`
- **Failover** — load-balance and fail over across multiple equivalent endpoints (`with_endpoints`, tonic `balance_list`)
- **Typed provider** — fluent builder API for every native contract operation
- **Filler chain** — automatic fee-limit and signing, plus optional explicit TAPOS overrides (mirrors alloy's `JoinFill`)
- **TRX / TRC10 / TRC20** — transfers, balance queries, and token metadata
- **Staking** — Stake 2.0 (freeze, unfreeze, delegate, undelegate, claim rewards) and Stake 1.0 legacy (`freeze_balance_v1`, `unfreeze_balance_v1`)
- **HD wallets** — BIP-39 mnemonic generation and BIP-44 key derivation (`signer-mnemonic` feature, TRON coin type 195)
- **Keystore** — Web3 Secret Storage V3 encrypt/decrypt (`signer-keystore` feature, compatible with TronLink and gotron-sdk)
- **AWS KMS** — sign with a key that never leaves the HSM (`signer-aws` feature, `AwsSigner`)
- **`tron_sol!` macro** — type-safe contract bindings with typed call/event builders and JSON ABI file path support (superset of alloy's `sol!`)
- **TRC721** — `Trc721Instance`: `transfer_from`, `approve`, `owner_of`, `token_uri`, and standard ERC-721 queries
- **Contract deploy & call** — native `TronAbi` metadata with an Alloy `JsonAbi` bridge, `DeployBuilder`, dynamic calls, and energy estimation
- **Event decoding** — decode and filter logs with `SolEvent`
- **Votes & account management** — SR voting, account activation, name and permission updates
- **Super representatives** — `WitnessApi`: become SR, update URL, update brokerage ratio
- **Governance** — `GovernanceApi`: list, query, submit, approve, and cancel chain-parameter proposals
- **TRC10 extended** — participate in ICOs, release frozen supply, update token metadata, look up by name

## Sponsors

tronz is grateful to the organizations that support its ongoing development
and maintenance.

<p align="center">
  <a href="https://catfee.io">
    <img src="./assets/sponsors/catfee.png" alt="CatFee logo" width="112">
  </a>
  <br>
  <strong><a href="https://catfee.io">CatFee</a></strong>
  <br>
  <sub>Project Sponsor</sub>
</p>

## Installation

```sh
cargo add tronz
```

Or add it to your `Cargo.toml` manually:

```toml
[dependencies]
tronz = "0.4"
```

Optional features:

| Feature | Adds |
|---|---|
| `signer-mnemonic` | BIP-39 mnemonic generation + BIP-44 HD derivation (`MnemonicBuilder`) |
| `signer-keystore` | Web3 Secret Storage V3 encrypt/decrypt (`LocalSigner::encrypt_keystore`, `decrypt_keystore`) |
| `signer-aws` | AWS KMS signer (`AwsSigner`) — the private key never leaves the HSM |
| `provider-grpc` | gRPC transport without TLS — use for local or private nodes |

## Quick start

### Read the latest block

```rust,no_run
use tronz::{ProviderBuilder, TronProvider, TRONGRID_MAINNET};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = ProviderBuilder::new()
        .on_grpc(TRONGRID_MAINNET)
        .await?;

    let block = provider.get_now_block().await?;
    println!("block #{} at {}ms", block.number, block.timestamp);
    Ok(())
}
```

### Send TRX

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, TronProvider, Trx, TRONGRID_NILE};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let signer = LocalSigner::from_hex("YOUR_PRIVATE_KEY")?;
    let to = "TRecipientAddress".parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .on_grpc(TRONGRID_NILE)
        .await?;

    let pending = provider
        .send_trx()
        .to(to)
        .amount(Trx::from_sun(1_000_000)?) // 1 TRX
        .send()
        .await?;

    let receipt = pending.get_receipt().await?;
    println!("confirmed in block #{}", receipt.block_number);
    Ok(())
}
```

### Call a TRC20 contract

```rust,no_run
use tronz::{ProviderBuilder, TronProvider, TRONGRID_MAINNET};
use tronz::contract::Trc20Ext as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = ProviderBuilder::new()
        .on_grpc(TRONGRID_MAINNET)
        .await?;

    // USDT on mainnet
    let usdt = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse()?;
    let holder = "THoldersAddress".parse()?;

    let token = provider.trc20(usdt);
    let balance = token.balance_of(holder).await?;
    let decimals = token.decimals().await?;

    println!("balance: {} (decimals: {})", balance, decimals);
    Ok(())
}
```

### Stake TRX and delegate energy

```rust,no_run
use tronz::{LocalSigner, ProviderBuilder, TronProvider, ResourceCode, TRONGRID_NILE, parse_trx};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let signer = LocalSigner::from_hex("YOUR_PRIVATE_KEY")?;
    let receiver = "TReceiverAddress".parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .with_signer(signer)
        .on_grpc(TRONGRID_NILE)
        .await?;

    // Freeze 100 TRX for energy
    provider
        .freeze_balance()
        .amount(parse_trx("100")?)
        .resource(ResourceCode::Energy)
        .send()
        .await?
        .get_receipt()
        .await?;

    // Delegate the energy to another account
    provider
        .delegate_resource()
        .resource(ResourceCode::Energy)
        .amount(parse_trx("100")?)
        .to(receiver)
        .send()
        .await?
        .get_receipt()
        .await?;

    Ok(())
}
```

### Derive a signer from a mnemonic phrase

```rust,no_run
use tronz::{MnemonicBuilder, TronSigner, coins_bip39::English};

fn main() -> anyhow::Result<()> {
    let phrase = "abandon abandon abandon abandon abandon abandon \
                  abandon abandon abandon abandon abandon about";

    // Default path: m/44'/195'/0'/0/0 (TRON BIP-44 coin type 195)
    let signer = MnemonicBuilder::<English>::default()
        .phrase(phrase)
        .index(0)?
        .build()?;
    println!("address: {}", signer.address());

    // Generate a fresh random 24-word mnemonic
    let (signer, phrase) = MnemonicBuilder::<English>::default()
        .word_count(24)
        .build_random()?;
    println!("new phrase: {phrase}");
    println!("address:    {}", signer.address());
    Ok(())
}
```

Requires the `signer-mnemonic` feature.

### Encrypt and decrypt a keystore

```rust,no_run
use tronz::{LocalSigner, TronSigner};

fn main() -> anyhow::Result<()> {
    let signer = LocalSigner::from_hex("YOUR_PRIVATE_KEY")?;

    // Encrypt to a JSON file (scrypt N=2^18, AES-128-CTR)
    let dir = std::path::Path::new("/tmp");
    let path = signer.encrypt_keystore(dir, "my-password")?;
    println!("saved: {}", path.display());

    // Decrypt back
    let recovered = LocalSigner::decrypt_keystore(&path, "my-password")?;
    assert_eq!(signer.address(), recovered.address());
    Ok(())
}
```

Requires the `signer-keystore` feature. The format is compatible with TronLink and gotron-sdk.

### Query governance proposals

```rust,no_run
use tronz::{ProviderBuilder, TRONGRID_MAINNET};
use tronz::providers::ext::GovernanceApi as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = ProviderBuilder::new()
        .on_grpc(TRONGRID_MAINNET)
        .await?;

    let proposals = provider.list_proposals().await?;
    for p in &proposals {
        println!("proposal #{}: {:?}", p.proposal_id, p.state);
    }

    let p = provider.get_proposal_by_id(1).await?;
    println!("proposal #1 parameters: {:?}", p.parameters);
    Ok(())
}
```

### List super representatives

```rust,no_run
use tronz::{ProviderBuilder, TronProvider, TRONGRID_MAINNET};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = ProviderBuilder::new()
        .on_grpc(TRONGRID_MAINNET)
        .await?;

    let mut witnesses = provider.list_witnesses().await?;
    witnesses.sort_by_key(|w| std::cmp::Reverse(w.vote_count));
    for w in witnesses.iter().take(5) {
        println!("{}: {} votes", w.address, w.vote_count);
    }
    Ok(())
}
```

### Query solidified (irreversible) state

`SolidityProvider` talks to a TRON **SolidityNode** (`WalletSolidity`), which only
serves state confirmed by 2/3+ of the super representatives — i.e. irreversible.
It is read-only by construction: no signer, no fillers, no broadcast. Use it when
finality matters (exchange credits, settlement) and poll `wait_for_success` to
block until a transaction has solidified and its execution succeeded.

```rust,no_run
use tronz::{SolidityProvider, TRONGRID_MAINNET_SOLIDITY};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let solidity = SolidityProvider::connect(TRONGRID_MAINNET_SOLIDITY).await?;

    // Latest solidified head.
    let head = solidity.get_now_block().await?;
    println!("solidified head: {}", head.number);

    // Block until a broadcast transaction is solidified *and* succeeded.
    let tx_id = std::env::var("TRON_TX_ID")?.parse()?;
    let receipt = solidity.wait_for_success(tx_id).await?;
    println!("solidified in block {}", receipt.block_number);
    Ok(())
}
```

After broadcasting on a FullNode you can bridge straight to solidification via the
pending handle — pass the `SolidityProvider` to poll:

```rust,ignore
let pending = provider.send_trx().to(to).amount(amount).send().await?;
let receipt = pending.await_solidified_success(&solidity).await?; // waits for irreversibility
```

## Crates

| Crate | Description |
|---|---|
| [`tronz`](https://crates.io/crates/tronz) | Meta-crate — re-exports everything |
| [`tronz-primitives`](https://crates.io/crates/tronz-primitives) | `Address`, `Trx`, `ResourceCode`, `RecoverableSignature` |
| [`tronz-abi`](https://crates.io/crates/tronz-abi) | Native TRON ABI metadata and optional Alloy `JsonAbi` conversion |
| [`tronz-signer`](https://crates.io/crates/tronz-signer) | `TronSigner` trait and `LocalSigner` (in-memory secp256k1) |
| [`tronz-provider`](https://crates.io/crates/tronz-provider) | FullNode and SolidityNode transports/providers, fillers, domain types, extension traits |
| [`tronz-contract`](https://crates.io/crates/tronz-contract) | `ContractInstance`, `DeployBuilder`, TRC20 bindings, event decoding |
| [`tronz-sol-macro`](https://crates.io/crates/tronz-sol-macro) | `tron_sol!` procedural macro |
| [`tronz-signer-aws`](https://crates.io/crates/tronz-signer-aws) | AWS KMS signer (`signer-aws` feature) |

## Extension traits

Import these to unlock additional methods on any provider:

| Trait | Import | Methods |
|---|---|---|
| `Trc10Api` | `use tronz::providers::ext::Trc10Api as _` | issue, transfer, balance, participate, update, look up by name |
| `WitnessApi` | `use tronz::providers::ext::WitnessApi as _` | list SRs, brokerage, become SR, update URL/brokerage |
| `GovernanceApi` | `use tronz::providers::ext::GovernanceApi as _` | list/fetch proposals, submit, approve, cancel |
| `ExchangeApi` | `use tronz::providers::ext::ExchangeApi as _` | create exchange, inject/withdraw liquidity, trade |
| `MarketApi` | `use tronz::providers::ext::MarketApi as _` | market orders, sell/cancel, query prices |

## Examples

42 runnable examples are in [throgxyz/examples](https://github.com/throgxyz/examples), organized by category. All target the Nile testnet.

```bash
git clone https://github.com/throgxyz/examples
cd examples

# Read-only queries (no key needed)
cargo run -p examples-queries --example query
cargo run -p examples-queries --example list_witnesses
cargo run -p examples-queries --example governance_list

# Send TRX on Nile testnet
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx

# TRC20 balance + transfer
TRON_PRIVATE_KEY=<hex> cargo run -p examples-trc20 --example trc20

# Stake 2.0: freeze + delegate + claim rewards
TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example stake

# TRC10: issue a new token
TRON_PRIVATE_KEY=<hex> cargo run -p examples-trc10 --example trc10_issue

# Deploy and call a smart contract
TRON_PRIVATE_KEY=<hex> cargo run -p examples-contracts --example contract_deploy

# HD wallet: derive from mnemonic
cargo run -p examples-signers --example signer_mnemonic
```

## Endpoints

| Network | Constant | Endpoint |
|---|---|---|
| Mainnet (TLS) | `TRONGRID_MAINNET` | `https://grpc.trongrid.io:443` |
| Mainnet SolidityNode | `TRONGRID_MAINNET_SOLIDITY` | `http://grpc.trongrid.io:50052` |
| Nile testnet | `TRONGRID_NILE` | `http://grpc.nile.trongrid.io:50051` |
| Nile SolidityNode | `TRONGRID_NILE_SOLIDITY` | `http://grpc.nile.trongrid.io:50061` |

```rust,no_run
use tronz::{
    TRONGRID_MAINNET, TRONGRID_MAINNET_SOLIDITY, TRONGRID_NILE,
    TRONGRID_NILE_SOLIDITY,
};
```

## Minimum Supported Rust Version

**1.91.1** (Rust 2024 edition).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
