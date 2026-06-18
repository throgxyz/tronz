# tronz-signer

Signing traits and local key signer for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

## Overview

[`TronSigner`] is the core trait — anything that can produce a recoverable
secp256k1 signature over a 32-byte transaction hash. [`LocalSigner`] is the
default in-memory implementation backed by a `k256` private key.

Mnemonic and keystore support are available behind feature flags; future
hardware-wallet signers can implement the same trait without changing the
provider or contract layers.

## Usage

```rust,ignore
use tronz_signer::{LocalSigner, TronSigner};

let signer = LocalSigner::from_hex("0xdeadbeef...")?;
println!("address: {}", signer.address());

let signature = signer.sign_hash(tx_hash).await?;
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
