//! BIP-39 mnemonic + BIP-44 HD key derivation for TRON.
//!
//! Shows how to:
//! - Derive a signer from an existing 12/24-word mnemonic phrase
//! - Generate a fresh random mnemonic
//! - Derive multiple accounts from the same phrase using different indices
//!
//! No network access required.
//!
//! ```bash
//! cargo run -p examples-signers --example signer_mnemonic
//! ```
//!
//! WARNING: This example prints private keys to stdout — only use it for
//! throwaway testnet keys.

use tronz::{MnemonicBuilder, TronSigner, coins_bip39::English};

fn main() -> anyhow::Result<()> {
    // ── 1. Derive from an existing phrase ─────────────────────────────────────

    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    // Default path: m/44'/195'/0'/0/0  (TRON coin type = 195)
    let signer = MnemonicBuilder::<English>::default()
        .phrase(phrase)
        .index(0)?
        .build()?;

    println!("=== Derive from phrase ===");
    println!("  phrase  : {phrase}");
    println!("  index   : 0");
    println!("  path    : m/44'/195'/0'/0/0");
    println!("  address : {}", signer.address());

    // ── 2. Multiple accounts from the same phrase ──────────────────────────────

    println!("\n=== First 3 accounts from same phrase ===");
    for i in 0u32..3 {
        let s = MnemonicBuilder::<English>::default()
            .phrase(phrase)
            .index(i)?
            .build()?;
        println!("  index {i}: {}", s.address());
    }

    // ── 3. Efficient multi-account derivation via parent key ───────────────────

    // build_parent_key() derives once to m/44'/195'/0'/0, then child() is cheap.
    let parent = MnemonicBuilder::<English>::default()
        .phrase(phrase)
        .build_parent_key()?;

    println!("\n=== Same 3 accounts via parent key (more efficient) ===");
    for i in 0u32..3 {
        let s = parent.child(i)?.signer()?;
        println!("  index {i}: {}", s.address());
    }

    // ── 4. Generate a fresh random 24-word mnemonic ────────────────────────────

    let (new_signer, new_phrase) = MnemonicBuilder::<English>::default()
        .word_count(24)
        .build_random()?;

    println!("\n=== Random 24-word mnemonic ===");
    println!("  phrase  : {new_phrase}");
    println!("  address : {}", new_signer.address());
    println!("  words   : {}", new_phrase.split_whitespace().count());

    // ── 5. Optional BIP-39 passphrase ─────────────────────────────────────────

    let with_pass = MnemonicBuilder::<English>::default()
        .phrase(phrase)
        .password("my-passphrase")
        .index(0)?
        .build()?;

    println!("\n=== With BIP-39 passphrase ===");
    println!("  address (no passphrase) : {}", signer.address());
    println!("  address (with pass)     : {}", with_pass.address());
    println!(
        "  different addresses     : {}",
        signer.address() != with_pass.address()
    );

    println!("\n=== Save phrase to fund on Nile testnet ===");
    println!("  1. Write down the phrase and keep it safe.");
    println!(
        "  2. Get Nile TRX: https://nileex.io/ → send to {}",
        new_signer.address()
    );

    Ok(())
}
