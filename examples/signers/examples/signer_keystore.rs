//! Web3 Secret Storage V3 keystore — encrypt and decrypt a TRON private key.
//!
//! Shows how to:
//! - Encrypt a private key to a password-protected JSON file
//! - Decrypt the file back to a working signer
//! - Verify the address is preserved round-trip
//!
//! No network access required.
//!
//! ```bash
//! cargo run -p examples-signers --example signer_keystore
//! ```
//!
//! The keystore format is compatible with TronLink, go-ethereum, and gotron-sdk.
//! It stores the TRON address in base58check format (not Ethereum hex).

use tronz::{LocalSigner, TronSigner};

fn main() -> anyhow::Result<()> {
    // ── 1. Create a signer from a known private key ───────────────────────────

    let private_key = "b5a4cea271ff424d7c31dc12a3e43e401df7a40d7412a15750f3f0b6b5449a28";
    let signer = LocalSigner::from_hex(private_key)?;

    println!("=== Original signer ===");
    println!("  address : {}", signer.address());

    // ── 2. Encrypt to a temp directory ────────────────────────────────────────

    let dir = tempfile::tempdir()?;
    let password = "my-secure-password";

    println!("\n=== Encrypting keystore ===");
    println!("  password : {password}");

    let path = signer.encrypt_keystore(dir.path(), password)?;
    println!("  saved to : {}", path.display());

    // ── 3. Inspect the JSON ───────────────────────────────────────────────────

    let json = std::fs::read_to_string(&path)?;
    let ks: tronz::KeystoreFile = serde_json::from_str(&json)?;

    println!("\n=== Keystore contents ===");
    println!("  version  : {}", ks.version);
    println!("  id       : {}", ks.id);
    println!("  address  : {}", ks.address);
    println!(
        "  kdf      : {} (N={})",
        ks.crypto.kdf, ks.crypto.kdfparams.n
    );
    println!("  cipher   : {}", ks.crypto.cipher);

    // ── 4. Decrypt and verify ─────────────────────────────────────────────────

    println!("\n=== Decrypting ===");
    let recovered = LocalSigner::decrypt_keystore(&path, password)?;
    println!("  recovered address : {}", recovered.address());

    assert_eq!(
        signer.address(),
        recovered.address(),
        "round-trip address mismatch"
    );
    println!("  addresses match   : true");

    // ── 5. Wrong password gives a clear error ─────────────────────────────────

    let err = LocalSigner::decrypt_keystore(&path, "wrong-password").unwrap_err();
    println!("\n=== Wrong password ===");
    println!("  error : {err}");

    Ok(())
}
