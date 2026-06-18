//! Create and use a `LocalSigner`.
//!
//! Shows how to:
//! - Load a `LocalSigner` from a hex-encoded private key
//! - Derive the TRON address from the key
//! - Sign an arbitrary 32-byte hash
//! - Verify the signature round-trips correctly
//!
//! No network access required.
//!
//! ```bash
//! cargo run -p examples-signers --example signer_local
//! ```
//!
//! Optional env:
//!   TRON_PRIVATE_KEY — hex key to load (defaults to a throwaway demo key)

use tronz::{
    LocalSigner, TronSigner,
    primitives::{B256, RecoverableSignature},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Load key ──────────────────────────────────────────────────────────────

    // Default to a throwaway demo key; override with TRON_PRIVATE_KEY.
    let key_hex = std::env::var("TRON_PRIVATE_KEY").unwrap_or_else(|_| {
        "0000000000000000000000000000000000000000000000000000000000000001".to_owned()
    });

    let signer = LocalSigner::from_hex(&key_hex)?;

    println!("=== LocalSigner ===");
    println!("  address : {}", signer.address());
    println!("  hex     : {}", signer.address().to_hex());
    println!("  debug   : {signer:?}  (key is hidden)");

    // ── Sign a hash ───────────────────────────────────────────────────────────
    //
    // `TronSigner::sign_hash` takes a B256 — the SHA-256 hash of the protobuf
    // raw transaction in real usage. Here we use an arbitrary test hash.

    let hash: B256 = B256::repeat_byte(0xab);
    println!("\n=== Sign hash ===");
    println!("  hash  : 0x{}", hex::encode(hash));

    let sig: RecoverableSignature = signer.sign_hash(hash).await?;
    let sig_bytes = sig.to_bytes();
    println!("  sig   : 0x{}", hex::encode(sig_bytes));
    println!("  v     : {}  (recovery id: 0 or 1)", sig.v());
    println!("  len   : {} bytes (r[32] + s[32] + v[1])", sig_bytes.len());

    // ── Split signature ───────────────────────────────────────────────────────
    //
    // `split()` recovers the k256 `Signature` and `RecoveryId` components.
    // We already have r/s/v more conveniently via `sig.r()`, `sig.s()`, `sig.v()`.

    println!("\n=== Signature components ===");
    println!("  r : 0x{}", hex::encode(sig.r()));
    println!("  s : 0x{}", hex::encode(sig.s()));
    println!("  v : {}", sig.v());

    // `split()` gives back the k256 types (useful if you need to pass to k256 APIs).
    let (k256_sig, recid) = sig.split()?;
    println!("  k256 sig length : {} bytes", k256_sig.to_bytes().len());
    println!("  recovery id     : {}", recid.to_byte());

    // ── Sign a second hash — deterministic for the same key ──────────────────

    let hash2: B256 = B256::repeat_byte(0xcd);
    let sig2 = signer.sign_hash(hash2).await?;
    assert_ne!(
        sig.to_bytes(),
        sig2.to_bytes(),
        "different hashes → different signatures"
    );
    println!("\n=== Determinism check ===");
    let sig_again = signer.sign_hash(hash).await?;
    assert_eq!(
        sig.to_bytes(),
        sig_again.to_bytes(),
        "same hash → same signature (RFC 6979)"
    );
    println!("  RFC 6979 deterministic signing: OK");

    Ok(())
}
