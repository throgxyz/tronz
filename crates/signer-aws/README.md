# tronz-signer-aws

AWS KMS signer for the [tronz](https://github.com/throgxyz/tronz) TRON SDK.

## Overview

[`AwsSigner`] implements [`TronSigner`](tronz_signer::TronSigner) backed by an
AWS KMS **ECC_SECG_P256K1** asymmetric signing key. The private key never
leaves the HSM — signing is delegated to the KMS `Sign` API and the recovery
parity, which KMS does not return, is determined locally by trial recovery.

Because the public key is only known after querying KMS, [`AwsSigner::new`] is
async and fetches it once to derive and cache the TRON address. Only
asynchronous signing is supported; there is no
[`TronSignerSync`](tronz_signer::TronSignerSync) implementation.

The [`aws_sdk_kms`] client crate is re-exported so that callers do not have to
depend on a matching version themselves.

## Usage

```rust,no_run
use tronz_signer::TronSigner;
use tronz_signer_aws::{aws_sdk_kms, AwsSigner};
use tronz_primitives::B256;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
let client = aws_sdk_kms::Client::new(&config);

let signer = AwsSigner::new(client, "your-key-id".to_string()).await?;
println!("address: {}", signer.address());

let tx_hash = B256::ZERO;
let signature = signer.sign_hash(&tx_hash).await?;
# Ok(())
# }
```

A signer can be handed to a provider directly, or registered in a
[`TronWallet`](tronz_signer::TronWallet) alongside other credentials — which is
how a KMS-held key participates in a TRON multisig account, with the remaining
keys living elsewhere:

```rust,no_run
use tronz_signer::{LocalSigner, TronNetworkWallet, TronSigner, TronWallet};
use tronz_signer_aws::AwsSigner;
use tronz_primitives::B256;

# async fn example(kms_signer: AwsSigner, local: LocalSigner) -> Result<(), Box<dyn std::error::Error>> {
# let tx_hash = B256::ZERO;
let kms_key = kms_signer.address();
let local_key = local.address();

let mut wallet = TronWallet::new(kms_signer);
wallet.register_signer(local);

let signatures = wallet.sign_hash_with_many(&[kms_key, local_key], &tx_hash).await?;
# Ok(())
# }
```

## KMS key requirements

The KMS key must be created with:

- **Key type**: Asymmetric
- **Key spec**: ECC_SECG_P256K1
- **Key usage**: Sign and verify

## Optional features

| Feature | What it enables |
|---|---|
| `tip712` | TIP-712 typed-data signing through `sign_typed_data` and `sign_dynamic_typed_data` |

## Live test

A live integration test is included but gated behind `#[ignore]`. Set
`AWS_KEY_ID` and provide valid credentials, then run:

```sh
AWS_KEY_ID=<key-id> cargo test -p tronz-signer-aws -- --ignored
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.
