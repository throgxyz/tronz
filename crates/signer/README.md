# tronz-signer

Signing traits and local key signer for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

## Overview

[`TronSigner`] is the core trait — anything that can produce a recoverable
secp256k1 signature over a 32-byte transaction hash. [`LocalSigner`] is the
default in-memory implementation backed by a `k256` private key.

[`TronNetworkWallet`] is the provider-facing layer: a wallet holds one or more
credentials, keyed by each credential's own address, and signs with whichever
one a caller names. [`TronWallet`] is the standard implementation. This mirrors
Alloy's separation between low-level signers and network wallets.

A TRON account can authorize keys belonging to other addresses, so the key that
signs is not always the transaction's owner. Providers treat the owner as a
preference and fall back to the wallet's default credential; for an account
whose permission needs more than one signature, `sign_hash_with_many` collects
them all over a single transaction hash.

Mnemonic and keystore support extend `LocalSigner`; other signing backends can
implement the same trait without changing the provider or contract layers.

## Usage

```rust,ignore
use tronz_signer::{LocalSigner, TronNetworkWallet, TronSigner};

let signer = LocalSigner::from_hex("0xdeadbeef...")?;
println!("address: {}", signer.address());

let signature = signer.sign_hash(&tx_hash).await?;

// Providers accept a signer directly, or a wallet holding several credentials.
let mut wallet = tronz_signer::TronWallet::new(signer);
wallet.register_signer(other_signer);

// Multisig: one transaction hash, several of the wallet's keys.
let signatures = wallet.sign_hash_with_many(&[key_a, key_b], &tx_hash).await?;
```

## Optional features

| Feature | What it enables |
|---|---|
| `mnemonic` | BIP-39 phrases and BIP-44 HD derivation through `MnemonicBuilder` |
| `keystore` | Web3 Secret Storage V3 encryption and decryption through `LocalSigner` |
| `tip712` | TIP-712 typed-data signing through `sign_typed_data` and `sign_dynamic_typed_data` |

AWS KMS signing is provided separately by
[`tronz-signer-aws`](https://crates.io/crates/tronz-signer-aws).

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
