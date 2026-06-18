//! Generate a fresh random secp256k1 key pair.
//!
//! Creates a cryptographically secure random private key, derives the TRON
//! address, and prints the key material. In real usage you would save the
//! private key to secure storage.
//!
//! No network access required.
//!
//! ```bash
//! cargo run -p examples-signers --example signer_generate
//! ```
//!
//! WARNING: This example prints private keys to stdout — only use it for
//! throwaway testnet keys.

use k256::ecdsa::SigningKey;
use tronz::{LocalSigner, TronSigner, primitives::Address};

fn main() -> anyhow::Result<()> {
    // ── Generate key pair ─────────────────────────────────────────────────────

    // k256::ecdsa::SigningKey::random generates a cryptographically secure key
    // using the OS random number generator (getrandom).
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    let key_bytes: [u8; 32] = key.to_bytes().into();
    let key_hex = hex::encode(key_bytes);

    let signer = LocalSigner::from_bytes(&key_bytes)?;
    let address: Address = signer.address();

    println!("=== Generated key pair ===");
    println!("  private key : {key_hex}");
    println!("  address     : {address}  (base58check)");
    println!("  address hex : {}", address.to_hex());
    println!("  address evm : 0x{}", hex::encode(address.as_evm_bytes()));

    // ── Every run produces a different key ────────────────────────────────────

    let key2 = SigningKey::random(&mut rand::rngs::OsRng);
    let signer2 = LocalSigner::from_bytes(&key2.to_bytes().into())?;
    assert_ne!(signer.address(), signer2.address(), "fresh keys are unique");
    println!("\n  second run  : {}", signer2.address());
    println!("  unique      : {}", signer.address() != signer2.address());

    // ── How to fund the address on Nile testnet ───────────────────────────────

    println!("\n=== Next steps ===");
    println!("  1. Save the private key to a secure location.");
    println!("  2. Get Nile TRX from the faucet: https://nileex.io/");
    println!("     (send to: {address})");
    println!(
        "  3. Run examples with: TRON_PRIVATE_KEY={key_hex} cargo run -p examples-transfers --example transfer_trx"
    );

    Ok(())
}
