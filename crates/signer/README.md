# tronz-signer

Signing traits and local key signer for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

## Overview

[`TronSigner`] is the core trait — anything that can produce a recoverable
secp256k1 signature over a 32-byte transaction hash. [`LocalSigner`] is the
default in-memory implementation backed by a `k256` private key.

Mnemonic and keystore support extend `LocalSigner`; other signing backends can
implement the same trait without changing the provider or contract layers.

## Usage

```rust,ignore
use tronz_signer::{LocalSigner, TronSigner};

let signer = LocalSigner::from_hex("0xdeadbeef...")?;
println!("address: {}", signer.address());

let signature = signer.sign_hash(tx_hash).await?;
```

## Optional features

| Feature | What it enables |
|---|---|
| `mnemonic` | BIP-39 phrases and BIP-44 HD derivation through `MnemonicBuilder` |
| `keystore` | Web3 Secret Storage V3 encryption and decryption through `LocalSigner` |

AWS KMS signing is provided separately by
[`tronz-signer-aws`](https://crates.io/crates/tronz-signer-aws).

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
