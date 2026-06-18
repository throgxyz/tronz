# Signers

`TronSigner` signs a 32-byte transaction id and exposes the corresponding TRON
address. The default implementation is `LocalSigner`, backed by a secp256k1
private key.

## Local Signer

```rust,no_run
use tronz::{LocalSigner, TronSigner};

# fn run() -> tronz::signers::SignerError {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX")?;
println!("address: {}", signer.address());
# Ok(()) }
```

## Mnemonics

Enable `signer-mnemonic` to derive TRON keys from BIP-39 phrases. TRON uses
BIP-44 coin type `195`.

```rust,no_run
use tronz::{MnemonicBuilder, TronSigner, coins_bip39::English};

# fn run() -> tronz::signers::SignerError {
let signer = MnemonicBuilder::<English>::default()
    .phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
    .index(0)?
    .build()?;
println!("{}", signer.address());
# Ok(()) }
```

## Keystores

Enable `signer-keystore` to read and write Web3 Secret Storage V3 files:

```rust,no_run
use tronz::{LocalSigner, TronSigner};

# fn run() -> tronz::signers::SignerError {
let signer = LocalSigner::from_hex("PRIVATE_KEY_HEX")?;
let path = signer.encrypt_keystore("/tmp", "password")?;
let recovered = LocalSigner::decrypt_keystore(path, "password")?;
assert_eq!(signer.address(), recovered.address());
# Ok(()) }
```

Never commit real private keys, mnemonics, keystore files, or passwords.
