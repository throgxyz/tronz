# tronz-signer-aws

AWS KMS signer for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

## Overview

[`AwsSigner`] implements [`TronSigner`] backed by an AWS KMS
**ECC_SECG_P256K1** asymmetric signing key. The private key never leaves the
HSM — signing is delegated to the KMS `Sign` API and the recovery parity is
determined locally by trial recovery.

## Usage

```rust,ignore
use aws_config::BehaviorVersion;
use tronz_signer_aws::AwsSigner;
use tronz_signer::TronSigner;

let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
let client = aws_sdk_kms::Client::new(&config);

let signer = AwsSigner::new(client, "your-key-id".to_string()).await?;
println!("address: {}", signer.address());

let signature = signer.sign_hash(tx_hash).await?;
```

## KMS key requirements

The KMS key must be created with:

- **Key type**: Asymmetric
- **Key spec**: ECC_SECG_P256K1
- **Key usage**: Sign and verify

## Live test

A live integration test is included but gated behind `#[ignore]`. Set
`AWS_KEY_ID` and provide valid credentials, then run:

```sh
AWS_KEY_ID=<key-id> cargo test -p tronz-signer-aws -- --ignored
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
