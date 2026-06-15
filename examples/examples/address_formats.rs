//! TRON address format conversions.
//!
//! Demonstrates how to parse and convert between all three address
//! representations:
//!
//! - **base58check** (`T…`) — the human-readable form shown in wallets and block explorers.
//! - **hex** (`41…`) — the raw 21-byte representation prefixed with `0x41`.
//! - **EVM** (20-byte) — the body without the `0x41` prefix, used for ABI encoding (e.g. TRC20
//!   contract calls).
//!
//! Also shows how to derive a TRON address from a private key using
//! `LocalSigner`.
//!
//! No network access required.
//!
//! ```bash
//! cargo run -p examples --example address_formats
//! ```

use tronz::{
    LocalSigner, TronSigner,
    primitives::{ADDRESS_PREFIX, Address},
};

fn main() -> anyhow::Result<()> {
    // ── Parse base58check ─────────────────────────────────────────────────────

    // The USDT (TRC20) contract address, commonly used in examples.
    let b58 = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";
    let addr = b58.parse::<Address>()?;

    println!("=== Address representations ===");
    println!("  base58  : {}", addr.to_base58());
    println!("  hex     : {}", addr.to_hex());
    println!("  evm     : 0x{}", hex::encode(addr.as_evm_bytes()));
    println!(
        "  prefix  : 0x{:02x} (must be 0x{:02x})",
        addr.as_bytes()[0],
        ADDRESS_PREFIX
    );

    // ── Parse hex ────────────────────────────────────────────────────────────

    let hex_str = "41a614f803b6fd780986a42c78ec9c7f77e6ded13c";
    let from_hex = hex_str.parse::<Address>()?;
    assert_eq!(from_hex, addr, "hex and base58 parsed the same address");
    println!("\n  hex  → base58 : {}", from_hex.to_base58());

    // ── Hex with 0x prefix ────────────────────────────────────────────────────

    let hex_0x = format!("0x{hex_str}");
    let from_0x = hex_0x.parse::<Address>()?;
    assert_eq!(from_0x, addr);
    println!("  0x…  → base58 : {}", from_0x.to_base58());

    // ── Construct from EVM bytes ──────────────────────────────────────────────

    let evm_bytes: [u8; 20] = addr.as_evm_bytes().to_owned();
    let from_evm = Address::from_evm_bytes(evm_bytes);
    assert_eq!(from_evm, addr);
    println!("  evm  → base58 : {}", from_evm.to_base58());

    // ── Bridge to alloy ───────────────────────────────────────────────────────
    //
    // tronz::Address and alloy_primitives::Address are interconvertible.
    // Use the alloy form when passing into ABI-encoding helpers (e.g. TRC20
    // `balanceOf` calls where the argument type is `address`).

    let alloy_addr: alloy_primitives::Address = addr.into();
    let back: Address = alloy_addr.into();
    assert_eq!(back, addr);
    println!("\n=== alloy bridge ===");
    println!("  tronz  : {}", addr);
    println!("  alloy  : {alloy_addr}  (EVM / checksum form)");
    println!("  round-trip OK: {}", back == addr);

    // ── Derive from private key ───────────────────────────────────────────────
    //
    // Address derivation: keccak256(uncompressed_pubkey[1..])[12..]
    // This is identical to Ethereum — only the 0x41 prefix differs.

    // Throwaway demo key — never use this in production.
    let demo_key = "0000000000000000000000000000000000000000000000000000000000000001";
    let signer = LocalSigner::from_hex(demo_key)?;
    let derived = signer.address();
    println!("\n=== Derive address from private key ===");
    println!("  private key : {demo_key}");
    println!("  address     : {derived}");
    println!("  hex         : {}", derived.to_hex());

    Ok(())
}
